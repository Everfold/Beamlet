// 目标选定(Selection::Any / Exact)、定向调用与三态提交裁决(Outcome)协议模型.
module routing {
  type SelectionKind =
    | SelectAny
    | SelectExact(int)

  type OpEntry =
    | OpNone
    | OpAwaiting({ attemptId: int, targetWorker: int })
    | OpClosed({ attemptId: int, reportId: int, worker: int })

  type SubOutcome =
    | SubIdle
    | SubRejected
    | SubUnresolved({ opId: int, attemptId: int, worker: int })
    | SubCompleted({ opId: int, attemptId: int, reportId: int, worker: int })

  type CallMsg = {
    opId: int,
    attemptId: int,
    worker: int,
  }

  type ReportMsg = {
    reportId: int,
    opId: int,
    attemptId: int,
    worker: int,
  }

  pure val WORKERS = Set(1, 2)
  pure val SUBMISSIONS = Set(1)
  pure val BOGUS_ATTEMPT = 9
  pure val BOGUS_REPORT = 9

  var workerRoutable: int -> bool
  var opEntry: int -> OpEntry
  var subOutcome: int -> SubOutcome
  var subSelection: int -> SelectionKind
  var attemptCount: int -> int
  var inFlightCalls: Set[CallMsg]
  var inFlightReports: Set[ReportMsg]
  var authenticReports: Set[ReportMsg]

  action init: bool = all {
    workerRoutable' = WORKERS.mapBy(_ => true),
    opEntry' = SUBMISSIONS.mapBy(_ => OpNone),
    subOutcome' = SUBMISSIONS.mapBy(_ => SubIdle),
    subSelection' = SUBMISSIONS.mapBy(_ => SelectAny),
    attemptCount' = SUBMISSIONS.mapBy(_ => 0),
    inFlightCalls' = Set(),
    inFlightReports' = Set(),
    authenticReports' = Set(),
  }

  action toggleWorkerRoutable(target: int): bool = all {
    workerRoutable' = workerRoutable.put(target, not(workerRoutable.get(target))),
    opEntry' = opEntry,
    subOutcome' = subOutcome,
    subSelection' = subSelection,
    attemptCount' = attemptCount,
    inFlightCalls' = inFlightCalls,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
  }

  // Exact 提交: 目标不可路由时直接裁决 Rejected 且不创建 Operation, 绝不回退其他候选.
  action submitExact(subId: int, target: int): bool = all {
    subOutcome.get(subId) == SubIdle,
    opEntry.get(subId) == OpNone,
    subSelection' = subSelection.put(subId, SelectExact(target)),
    if (workerRoutable.get(target)) all {
      opEntry' = opEntry.put(subId, OpAwaiting({ attemptId: subId, targetWorker: target })),
      subOutcome' = subOutcome,
      attemptCount' = attemptCount.put(subId, attemptCount.get(subId) + 1),
      inFlightCalls' = inFlightCalls.union(Set({ opId: subId, attemptId: subId, worker: target })),
    } else all {
      opEntry' = opEntry,
      subOutcome' = subOutcome.put(subId, SubRejected),
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
    },
    workerRoutable' = workerRoutable,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
  }

  // Any 提交: 从当前可路由候选中任选其一, 无候选时直接裁决 Rejected.
  action submitAny(subId: int): bool = all {
    subOutcome.get(subId) == SubIdle,
    opEntry.get(subId) == OpNone,
    subSelection' = subSelection.put(subId, SelectAny),
    if (WORKERS.exists(w => workerRoutable.get(w))) {
      nondet picked = WORKERS.filter(w => workerRoutable.get(w)).oneOf()
      all {
        opEntry' = opEntry.put(subId, OpAwaiting({ attemptId: subId, targetWorker: picked })),
        subOutcome' = subOutcome,
        attemptCount' = attemptCount.put(subId, attemptCount.get(subId) + 1),
        inFlightCalls' = inFlightCalls.union(Set({ opId: subId, attemptId: subId, worker: picked })),
      }
    } else all {
      opEntry' = opEntry,
      subOutcome' = subOutcome.put(subId, SubRejected),
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
    },
    workerRoutable' = workerRoutable,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
  }

  // Worker 正常执行并返回完整 Report (允许此时 Worker 已退出可路由状态).
  action workerReplyReport(call: CallMsg): bool = {
    val rep = {
      reportId: call.opId,
      opId: call.opId,
      attemptId: call.attemptId,
      worker: call.worker,
    }
    all {
      inFlightCalls.contains(call),
      inFlightCalls' = inFlightCalls.exclude(Set(call)),
      inFlightReports' = inFlightReports.union(Set(rep)),
      authenticReports' = authenticReports.union(Set(rep)),
      workerRoutable' = workerRoutable,
      opEntry' = opEntry,
      subOutcome' = subOutcome,
      subSelection' = subSelection,
      attemptCount' = attemptCount,
    }
  }

  // Worker 执行期失败或交付未达/超时: 裁决 Unresolved 且保持 OpAwaiting.
  action authorityTimeoutOrFailed(subId: int): bool = {
    val canResolve = match opEntry.get(subId) {
      | OpAwaiting(_) => subOutcome.get(subId) == SubIdle
      | _ => false
    }
    val nextOutcome = match opEntry.get(subId) {
      | OpAwaiting(a) => SubUnresolved({
          opId: subId,
          attemptId: a.attemptId,
          worker: a.targetWorker,
        })
      | _ => subOutcome.get(subId)
    }
    all {
      canResolve,
      subOutcome' = subOutcome.put(subId, nextOutcome),
      workerRoutable' = workerRoutable,
      opEntry' = opEntry,
      subSelection' = subSelection,
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
      inFlightReports' = inFlightReports,
      authenticReports' = authenticReports,
    }
  }

  // 注入错配或过时 Report.
  action injectMismatchedReport(subId: int, wrongWorker: int): bool = {
    val bogus = {
      reportId: BOGUS_REPORT,
      opId: subId,
      attemptId: BOGUS_ATTEMPT,
      worker: wrongWorker,
    }
    all {
      opEntry.get(subId) != OpNone,
      not(inFlightReports.contains(bogus)),
      inFlightReports' = inFlightReports.union(Set(bogus)),
      workerRoutable' = workerRoutable,
      opEntry' = opEntry,
      subOutcome' = subOutcome,
      subSelection' = subSelection,
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
      authenticReports' = authenticReports,
    }
  }

  // Authority 收取 Report: 核对 Operation、Attempt 与锁定目标一致, 不依赖当前可路由性.
  action authorityReceiveReport(rep: ReportMsg): bool = {
    val curOp = opEntry.get(rep.opId)
    val matched = match curOp {
      | OpAwaiting(a) => (rep.attemptId == a.attemptId and rep.worker == a.targetWorker)
      | _ => false
    }
    val nextOp =
      if (matched) OpClosed({ attemptId: rep.attemptId, reportId: rep.reportId, worker: rep.worker })
      else curOp
    val nextOutcome =
      if (matched and subOutcome.get(rep.opId) == SubIdle)
        SubCompleted({ opId: rep.opId, attemptId: rep.attemptId, reportId: rep.reportId, worker: rep.worker })
      else subOutcome.get(rep.opId)
    all {
      inFlightReports.contains(rep),
      inFlightReports' = inFlightReports.exclude(Set(rep)),
      opEntry' = opEntry.put(rep.opId, nextOp),
      subOutcome' = subOutcome.put(rep.opId, nextOutcome),
      workerRoutable' = workerRoutable,
      subSelection' = subSelection,
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
      authenticReports' = authenticReports,
    }
  }

  action step: bool = any {
    {
      nondet target = WORKERS.oneOf()
      toggleWorkerRoutable(target)
    },
    {
      nondet subId = SUBMISSIONS.oneOf()
      nondet target = WORKERS.oneOf()
      submitExact(subId, target)
    },
    {
      nondet subId = SUBMISSIONS.oneOf()
      submitAny(subId)
    },
    {
      nondet call = inFlightCalls.oneOf()
      all {
        inFlightCalls.size() > 0,
        workerReplyReport(call),
      }
    },
    {
      nondet subId = SUBMISSIONS.oneOf()
      nondet target = WORKERS.oneOf()
      injectMismatchedReport(subId, target)
    },
    {
      nondet subId = SUBMISSIONS.oneOf()
      authorityTimeoutOrFailed(subId)
    },
    {
      nondet rep = inFlightReports.oneOf()
      all {
        inFlightReports.size() > 0,
        authorityReceiveReport(rep),
      }
    },
  }

  // Exact 选择绝不回退, Rejected 时不创建 Operation.
  val exactNeverFallsBack: bool =
    SUBMISSIONS.forall(subId =>
      match subSelection.get(subId) {
        | SelectAny => true
        | SelectExact(w) => and {
            (subOutcome.get(subId) == SubRejected) implies (opEntry.get(subId) == OpNone),
            match opEntry.get(subId) {
              | OpNone => true
              | OpAwaiting(a) => a.targetWorker == w
              | OpClosed(c) => c.worker == w
            },
            match subOutcome.get(subId) {
              | SubIdle => true
              | SubRejected => true
              | SubUnresolved(u) => u.worker == w
              | SubCompleted(c) => c.worker == w
            },
          }
      }
    )

  // Rejected 不产生任何 Operation 或 Attempt.
  val rejectedCreatesNoOperation: bool =
    SUBMISSIONS.forall(subId =>
      (subOutcome.get(subId) == SubRejected) implies (opEntry.get(subId) == OpNone and attemptCount.get(subId) == 0)
    )

  // Completed 与 OpClosed 必定对应一条真实匹配的 Report.
  val completedRequiresAuthenticReport: bool =
    SUBMISSIONS.forall(subId => and {
      match opEntry.get(subId) {
        | OpClosed(c) => authenticReports.contains({
            reportId: c.reportId,
            opId: subId,
            attemptId: c.attemptId,
            worker: c.worker,
          })
        | _ => true
      },
      match subOutcome.get(subId) {
        | SubCompleted(c) => authenticReports.contains({
            reportId: c.reportId,
            opId: subId,
            attemptId: c.attemptId,
            worker: c.worker,
          })
        | _ => true
      },
    })

  // Unresolved 不得无证关闭 Operation; 错配报告永不关闭 Operation.
  val unresolvedKeepsOperationOpenUnlessReported: bool =
    SUBMISSIONS.forall(subId => and {
      attemptCount.get(subId) <= 1,
      match opEntry.get(subId) {
        | OpClosed(c) => c.attemptId != BOGUS_ATTEMPT and c.reportId != BOGUS_REPORT
        | _ => true
      },
      match subOutcome.get(subId) {
        | SubUnresolved(u) =>
            match opEntry.get(subId) {
              | OpNone => false
              | OpAwaiting(a) => a.attemptId == u.attemptId and a.targetWorker == u.worker
              | OpClosed(c) => authenticReports.contains({
                  reportId: c.reportId,
                  opId: subId,
                  attemptId: c.attemptId,
                  worker: c.worker,
                })
            }
        | _ => true
      },
    })

  val invariants: bool = and {
    exactNeverFallsBack,
    rejectedCreatesNoOperation,
    completedRequiresAuthenticReport,
    unresolvedKeepsOperationOpenUnlessReported,
  }

  val witnessExactCompleted: bool =
    match subOutcome.get(1) {
      | SubCompleted(_) =>
          match subSelection.get(1) {
            | SelectExact(_) => true
            | _ => false
          }
      | _ => false
    }

  val witnessExactRejectedWhileOtherRoutable: bool =
    subOutcome.get(1) == SubRejected and
      match subSelection.get(1) {
        | SelectExact(w) => WORKERS.exists(other => other != w and workerRoutable.get(other))
        | _ => false
      }

  val witnessUnresolvedKeepsAwaiting: bool =
    match subOutcome.get(1) {
      | SubUnresolved(_) =>
          match opEntry.get(1) {
            | OpAwaiting(_) => true
            | _ => false
          }
      | _ => false
    }

  val witnessCompletedAfterWorkerUnroutable: bool =
    match opEntry.get(1) {
      | OpClosed(c) => not(workerRoutable.get(c.worker))
      | _ => false
    }
}
