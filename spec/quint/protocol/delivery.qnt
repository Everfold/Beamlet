// L3 定向交付与失败语义协议模型.
// 对应 05-roles.md 与 Authority 编排: Exact 不回退, 超时返回 Unresolved 且保持 AwaitingReport, 仅接受真实匹配的 Report.
module delivery {
  type SelectionMode =
    | SelectAny
    | SelectExact(int)

  type OpEntry =
    | OpAbsent
    | OpAwaiting({ attemptId: int, targetWorker: int })
    | OpClosed({ attemptId: int, reportId: int, worker: int })

  type ReqStatus =
    | ReqIdle
    | ReqRejected
    | ReqUnresolved({ opId: int, attemptId: int, worker: int })
    | ReqCompleted({ opId: int, attemptId: int, reportId: int, worker: int })

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
  pure val REQUESTS = Set(1, 2)
  pure val BOGUS_ATTEMPT_ID = 9
  pure val BOGUS_REPORT_ID = 9

  var workerReady: int -> bool
  var opStates: int -> OpEntry
  var reqState: int -> ReqStatus
  var reqSelection: int -> SelectionMode
  var attemptCount: int -> int
  var inFlightCalls: Set[CallMsg]
  var inFlightReports: Set[ReportMsg]
  var authenticReports: Set[ReportMsg]

  action init: bool = all {
    workerReady' = WORKERS.mapBy(_ => true),
    opStates' = REQUESTS.mapBy(_ => OpAbsent),
    reqState' = REQUESTS.mapBy(_ => ReqIdle),
    reqSelection' = REQUESTS.mapBy(_ => SelectAny),
    attemptCount' = REQUESTS.mapBy(_ => 0),
    inFlightCalls' = Set(),
    inFlightReports' = Set(),
    authenticReports' = Set(),
  }

  // Worker 随时可能上线或失联/下线.
  action toggleWorkerReady(w: int): bool = all {
    workerReady' = workerReady.put(w, not(workerReady.get(w))),
    opStates' = opStates,
    reqState' = reqState,
    reqSelection' = reqSelection,
    attemptCount' = attemptCount,
    inFlightCalls' = inFlightCalls,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
  }

  // Exact 提交: 目标不可路由时直接返回 ReqRejected 且不创建 Operation, 绝不回退其他 Worker.
  action submitExact(reqId: int, target: int): bool = all {
    reqState.get(reqId) == ReqIdle,
    opStates.get(reqId) == OpAbsent,
    reqSelection' = reqSelection.put(reqId, SelectExact(target)),
    if (workerReady.get(target)) all {
      opStates' = opStates.put(reqId, OpAwaiting({ attemptId: reqId, targetWorker: target })),
      reqState' = reqState,
      attemptCount' = attemptCount.put(reqId, attemptCount.get(reqId) + 1),
      inFlightCalls' = inFlightCalls.union(Set({ opId: reqId, attemptId: reqId, worker: target })),
    } else all {
      opStates' = opStates,
      reqState' = reqState.put(reqId, ReqRejected),
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
    },
    workerReady' = workerReady,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
  }

  // Any 提交: 从当前就绪候选中挑选一个, 或全部不可用时拒绝.
  action submitAny(reqId: int): bool = all {
    reqState.get(reqId) == ReqIdle,
    opStates.get(reqId) == OpAbsent,
    reqSelection' = reqSelection.put(reqId, SelectAny),
    if (WORKERS.exists(w => workerReady.get(w))) {
      nondet picked = WORKERS.filter(w => workerReady.get(w)).oneOf()
      all {
        opStates' = opStates.put(reqId, OpAwaiting({ attemptId: reqId, targetWorker: picked })),
        reqState' = reqState,
        attemptCount' = attemptCount.put(reqId, attemptCount.get(reqId) + 1),
        inFlightCalls' = inFlightCalls.union(Set({ opId: reqId, attemptId: reqId, worker: picked })),
      }
    } else all {
      opStates' = opStates,
      reqState' = reqState.put(reqId, ReqRejected),
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
    },
    workerReady' = workerReady,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
  }

  // Worker 执行已交付调用并产出完整 Report (执行时不需要 Authority 端仍保持租约在线).
  action workerExecute(call: CallMsg): bool = {
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
      workerReady' = workerReady,
      opStates' = opStates,
      reqState' = reqState,
      reqSelection' = reqSelection,
      attemptCount' = attemptCount,
    }
  }

  // 网络中出现关联不匹配的陈旧或错误 Report.
  action injectBogusReport(opId: int, wrongWorker: int): bool = {
    val bogus = {
      reportId: BOGUS_REPORT_ID,
      opId: opId,
      attemptId: BOGUS_ATTEMPT_ID,
      worker: wrongWorker,
    }
    all {
      opStates.get(opId) != OpAbsent,
      not(inFlightReports.contains(bogus)),
      inFlightReports' = inFlightReports.union(Set(bogus)),
      workerReady' = workerReady,
      opStates' = opStates,
      reqState' = reqState,
      reqSelection' = reqSelection,
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
      authenticReports' = authenticReports,
    }
  }

  // Authority 等待报告超时: 返回 ReqUnresolved, 但保持 Operation 为 OpAwaiting, 不取消也不重发.
  action authorityTimeout(reqId: int): bool = {
    val canTimeout = match opStates.get(reqId) {
      | OpAwaiting(_) => reqState.get(reqId) == ReqIdle
      | _ => false
    }
    val nextReqStatus = match opStates.get(reqId) {
      | OpAwaiting(awaiting) => ReqUnresolved({
          opId: reqId,
          attemptId: awaiting.attemptId,
          worker: awaiting.targetWorker,
        })
      | _ => reqState.get(reqId)
    }
    all {
      canTimeout,
      reqState' = reqState.put(reqId, nextReqStatus),
      workerReady' = workerReady,
      opStates' = opStates,
      reqSelection' = reqSelection,
      attemptCount' = attemptCount,
      inFlightCalls' = inFlightCalls,
      inFlightReports' = inFlightReports,
      authenticReports' = authenticReports,
    }
  }

  // Authority 收取 Report: 核对 Operation, Attempt 与目标 Worker 身份, 不依赖 Worker 当前在线状态.
  action authorityReceiveReport(rep: ReportMsg): bool = {
    val restReports = inFlightReports.exclude(Set(rep))
    match opStates.get(rep.opId) {
      | OpAwaiting(awaiting) =>
          if (rep.attemptId == awaiting.attemptId and rep.worker == awaiting.targetWorker)
            all {
              opStates' = opStates.put(rep.opId, OpClosed({
                attemptId: rep.attemptId,
                reportId: rep.reportId,
                worker: rep.worker,
              })),
              reqState' =
                if (reqState.get(rep.opId) == ReqIdle)
                  reqState.put(rep.opId, ReqCompleted({
                    opId: rep.opId,
                    attemptId: rep.attemptId,
                    reportId: rep.reportId,
                    worker: rep.worker,
                  }))
                else reqState,
              inFlightReports' = restReports,
              workerReady' = workerReady,
              reqSelection' = reqSelection,
              attemptCount' = attemptCount,
              inFlightCalls' = inFlightCalls,
              authenticReports' = authenticReports,
            }
          else all {
            inFlightReports' = restReports,
            workerReady' = workerReady,
            opStates' = opStates,
            reqState' = reqState,
            reqSelection' = reqSelection,
            attemptCount' = attemptCount,
            inFlightCalls' = inFlightCalls,
            authenticReports' = authenticReports,
          }
      | _ => all {
          inFlightReports' = restReports,
          workerReady' = workerReady,
          opStates' = opStates,
          reqState' = reqState,
          reqSelection' = reqSelection,
          attemptCount' = attemptCount,
          inFlightCalls' = inFlightCalls,
          authenticReports' = authenticReports,
        }
    }
  }

  action step: bool = {
    nondet reqId = REQUESTS.oneOf()
    nondet w = WORKERS.oneOf()
    any {
      toggleWorkerReady(w),
      submitExact(reqId, w),
      submitAny(reqId),
      {
        nondet call = inFlightCalls.oneOf()
        all {
          inFlightCalls.size() > 0,
          workerExecute(call),
        }
      },
      injectBogusReport(reqId, 1),
      authorityTimeout(reqId),
      {
        nondet rep = inFlightReports.oneOf()
        all {
          inFlightReports.size() > 0,
          authorityReceiveReport(rep),
        }
      },
    }
  }

  // --- 安全不变量 (对应 05-roles.md 中的 INV-DLV-*) ---

  // INV-DLV-01: Exact 选择永不回退到其他 Worker, 拒绝时绝不创建 Operation.
  val inv_dlv_01_exact_never_falls_back: bool =
    REQUESTS.forall(reqId =>
      match reqSelection.get(reqId) {
        | SelectAny => true
        | SelectExact(w) => and {
            (reqState.get(reqId) == ReqRejected) implies (opStates.get(reqId) == OpAbsent),
            match opStates.get(reqId) {
              | OpAbsent => true
              | OpAwaiting(a) => a.targetWorker == w
              | OpClosed(c) => c.worker == w
            },
            match reqState.get(reqId) {
              | ReqIdle => true
              | ReqRejected => true
              | ReqUnresolved(u) => u.worker == w
              | ReqCompleted(c) => c.worker == w
            },
          }
      }
    )

  // INV-DLV-02: 每个 Operation 至多签发一次 Attempt, 返回 Unresolved 不得在无真实 Report 下关闭 Operation.
  val inv_dlv_02_unresolved_keeps_operation_open_or_validly_closed: bool =
    REQUESTS.forall(reqId => and {
      attemptCount.get(reqId) <= 1,
      match reqState.get(reqId) {
        | ReqUnresolved(u) =>
            match opStates.get(reqId) {
              | OpAbsent => false
              | OpAwaiting(a) => a.attemptId == u.attemptId and a.targetWorker == u.worker
              | OpClosed(c) => authenticReports.contains({
                  reportId: c.reportId,
                  opId: reqId,
                  attemptId: c.attemptId,
                  worker: c.worker,
                })
            }
        | _ => true
      },
    })

  // INV-DLV-03: 每个 OpClosed 和 ReqCompleted 均对应一条真实的 Worker Report.
  val inv_dlv_03_completed_has_authentic_report: bool =
    REQUESTS.forall(reqId => and {
      match opStates.get(reqId) {
        | OpClosed(c) => authenticReports.contains({
            reportId: c.reportId,
            opId: reqId,
            attemptId: c.attemptId,
            worker: c.worker,
          })
        | _ => true
      },
      match reqState.get(reqId) {
        | ReqCompleted(c) => authenticReports.contains({
            reportId: c.reportId,
            opId: reqId,
            attemptId: c.attemptId,
            worker: c.worker,
          })
        | _ => true
      },
    })

  // INV-DLV-04: 伪造或不匹配的 Report 永不关闭 Operation.
  val inv_dlv_04_mismatched_report_never_closes: bool =
    REQUESTS.forall(reqId =>
      match opStates.get(reqId) {
        | OpClosed(c) => c.attemptId != BOGUS_ATTEMPT_ID and c.reportId != BOGUS_REPORT_ID
        | _ => true
      }
    )

  val invariants: bool = and {
    inv_dlv_01_exact_never_falls_back,
    inv_dlv_02_unresolved_keeps_operation_open_or_validly_closed,
    inv_dlv_03_completed_has_authentic_report,
    inv_dlv_04_mismatched_report_never_closes,
  }

  // --- 可达性见证 ---

  val witnessExactSuccess: bool =
    REQUESTS.exists(reqId =>
      match reqState.get(reqId) {
        | ReqCompleted(_) =>
            match reqSelection.get(reqId) {
              | SelectExact(_) => true
              | _ => false
            }
        | _ => false
      }
    )

  val witnessExactNoFallback: bool =
    REQUESTS.exists(reqId =>
      reqState.get(reqId) == ReqRejected and
        match reqSelection.get(reqId) {
          | SelectExact(_) => true
          | _ => false
        }
    )

  val witnessTimeoutUnresolved: bool =
    REQUESTS.exists(reqId =>
      match reqState.get(reqId) {
        | ReqUnresolved(_) =>
            match opStates.get(reqId) {
              | OpAwaiting(_) => true
              | _ => false
            }
        | _ => false
      }
    )

  val witnessLateReportAfterWorkerOffline: bool =
    REQUESTS.exists(reqId =>
      match opStates.get(reqId) {
        | OpClosed(c) => not(workerReady.get(c.worker))
        | _ => false
      }
    )
}
