// Authority 与 Worker 的基础职责分工及单次尝试交付交互模型.
// 仅建模 Authority(持有台账) 与 Worker(无台账执行者) 以及二分结果契约.
module roles {
  type OpSlot =
    | Uncreated
    | ReadyOp
    | AwaitingOp({ attemptId: int, worker: int })
    | ClosedOp

  type DeliveryStatus =
    | DeliveryIdle
    | DeliveryNoReport
    | DeliveryReported

  type InFlightCall = {
    opId: int,
    attemptId: int,
    worker: int,
  }

  type ReportRecord = {
    reportId: int,
    opId: int,
    attemptId: int,
    worker: int,
  }

  pure val OPS = Set(1)
  pure val ATTEMPTS = Set(1, 2)
  pure val WORKERS = Set(1, 2)
  pure val BOGUS_ATTEMPT_ID = 9
  pure val BOGUS_REPORT_ID = 9

  var opSlot: int -> OpSlot
  var usedAttempts: Set[int]
  var issuedList: List[{ opId: int, attemptId: int, worker: int }]
  var workerOnline: int -> bool
  var inFlightCalls: Set[InFlightCall]
  var inFlightReports: Set[ReportRecord]
  var authenticReports: Set[ReportRecord]
  var acceptedReports: Set[ReportRecord]
  var lastDelivery: DeliveryStatus

  action init: bool = all {
    opSlot' = OPS.mapBy(_ => Uncreated),
    usedAttempts' = Set(),
    issuedList' = List(),
    workerOnline' = WORKERS.mapBy(_ => true),
    inFlightCalls' = Set(),
    inFlightReports' = Set(),
    authenticReports' = Set(),
    acceptedReports' = Set(),
    lastDelivery' = DeliveryIdle,
  }

  // Authority 创建 Operation (初始 Ready).
  action createOp(opId: int): bool = all {
    opSlot.get(opId) == Uncreated,
    opSlot' = opSlot.put(opId, ReadyOp),
    usedAttempts' = usedAttempts,
    issuedList' = issuedList,
    workerOnline' = workerOnline,
    inFlightCalls' = inFlightCalls,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
    acceptedReports' = acceptedReports,
    lastDelivery' = lastDelivery,
  }

  // Authority 签发未使用 Attempt 并向目标 Worker 投递.
  action dispatchDelivered(opId: int, attemptId: int, target: int): bool = all {
    opSlot.get(opId) == ReadyOp,
    not(usedAttempts.contains(attemptId)),
    opSlot' = opSlot.put(opId, AwaitingOp({ attemptId: attemptId, worker: target })),
    usedAttempts' = usedAttempts.union(Set(attemptId)),
    issuedList' = issuedList.append({ opId: opId, attemptId: attemptId, worker: target }),
    inFlightCalls' = inFlightCalls.union(Set({ opId: opId, attemptId: attemptId, worker: target })),
    workerOnline' = workerOnline,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
    acceptedReports' = acceptedReports,
    lastDelivery' = lastDelivery,
  }

  // 签发后未到达或被拒收: 不得关闭或重发, Operation 保持等待.
  action dispatchFailedBeforeArrival(opId: int, attemptId: int, target: int): bool = all {
    opSlot.get(opId) == ReadyOp,
    not(usedAttempts.contains(attemptId)),
    opSlot' = opSlot.put(opId, AwaitingOp({ attemptId: attemptId, worker: target })),
    usedAttempts' = usedAttempts.union(Set(attemptId)),
    issuedList' = issuedList.append({ opId: opId, attemptId: attemptId, worker: target }),
    lastDelivery' = DeliveryNoReport,
    workerOnline' = workerOnline,
    inFlightCalls' = inFlightCalls,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
    acceptedReports' = acceptedReports,
  }

  // Worker 执行收到的既有 Attempt 并产出完整 Report; 不得擅自签发新 Attempt.
  action workerProduceReport(call: InFlightCall): bool = {
    val rep = {
      reportId: call.attemptId,
      opId: call.opId,
      attemptId: call.attemptId,
      worker: call.worker,
    }
    all {
      inFlightCalls.contains(call),
      inFlightCalls' = inFlightCalls.exclude(Set(call)),
      inFlightReports' = inFlightReports.union(Set(rep)),
      authenticReports' = authenticReports.union(Set(rep)),
      opSlot' = opSlot,
      usedAttempts' = usedAttempts,
      issuedList' = issuedList,
      workerOnline' = workerOnline,
      acceptedReports' = acceptedReports,
      lastDelivery' = lastDelivery,
    }
  }

  // Worker 执行期失败: 不伪造 Report, Operation 保持等待.
  action workerFailExecution(call: InFlightCall): bool = all {
    inFlightCalls.contains(call),
    inFlightCalls' = inFlightCalls.exclude(Set(call)),
    lastDelivery' = DeliveryNoReport,
    opSlot' = opSlot,
    usedAttempts' = usedAttempts,
    issuedList' = issuedList,
    workerOnline' = workerOnline,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
    acceptedReports' = acceptedReports,
  }

  // 等待报告超时或断线: 结束本次等待, 保持 Operation 为 AwaitingReport.
  action authorityTimeoutOrDisconnect(opId: int): bool = {
    val awaiting = match opSlot.get(opId) {
      | AwaitingOp(_) => true
      | _ => false
    }
    all {
      awaiting,
      lastDelivery' = DeliveryNoReport,
      opSlot' = opSlot,
      usedAttempts' = usedAttempts,
      issuedList' = issuedList,
      workerOnline' = workerOnline,
      inFlightCalls' = inFlightCalls,
      inFlightReports' = inFlightReports,
      authenticReports' = authenticReports,
      acceptedReports' = acceptedReports,
    }
  }

  // 注入关联不匹配的错误或陈旧 Report.
  action injectMismatchedReport(opId: int, wrongWorker: int): bool = {
    val bogus = {
      reportId: BOGUS_REPORT_ID,
      opId: opId,
      attemptId: BOGUS_ATTEMPT_ID,
      worker: wrongWorker,
    }
    all {
      opSlot.get(opId) != Uncreated,
      not(inFlightReports.contains(bogus)),
      inFlightReports' = inFlightReports.union(Set(bogus)),
      opSlot' = opSlot,
      usedAttempts' = usedAttempts,
      issuedList' = issuedList,
      workerOnline' = workerOnline,
      inFlightCalls' = inFlightCalls,
      authenticReports' = authenticReports,
      acceptedReports' = acceptedReports,
      lastDelivery' = lastDelivery,
    }
  }

  // Authority 验收 Report: 仅校验身份与目标 Worker 匹配, 不依赖 Worker 当前是否在线.
  action authorityReceiveReport(rep: ReportRecord): bool = {
    val cur = opSlot.get(rep.opId)
    val valid = match cur {
      | AwaitingOp(a) => (rep.attemptId == a.attemptId and rep.worker == a.worker)
      | _ => false
    }
    val nextSlot = if (valid) ReadyOp else cur
    val nextAccepted = if (valid) acceptedReports.union(Set(rep)) else acceptedReports
    val nextDelivery = if (valid) DeliveryReported else lastDelivery
    all {
      inFlightReports.contains(rep),
      inFlightReports' = inFlightReports.exclude(Set(rep)),
      opSlot' = opSlot.put(rep.opId, nextSlot),
      acceptedReports' = nextAccepted,
      lastDelivery' = nextDelivery,
      usedAttempts' = usedAttempts,
      issuedList' = issuedList,
      workerOnline' = workerOnline,
      inFlightCalls' = inFlightCalls,
      authenticReports' = authenticReports,
    }
  }

  // Authority 仅在 ReadyOp (未等待或已验收 Report) 时可关闭 Operation.
  action authorityClose(opId: int): bool = {
    val cur = opSlot.get(opId)
    val canClose = (cur == ReadyOp or cur == ClosedOp)
    all {
      canClose,
      opSlot' = opSlot.put(opId, ClosedOp),
      usedAttempts' = usedAttempts,
      issuedList' = issuedList,
      workerOnline' = workerOnline,
      inFlightCalls' = inFlightCalls,
      inFlightReports' = inFlightReports,
      authenticReports' = authenticReports,
      acceptedReports' = acceptedReports,
      lastDelivery' = lastDelivery,
    }
  }

  // Worker 在线状态变化(用于验证离线后仍可验收已产出的合法 Report).
  action toggleWorkerOnline(target: int): bool = all {
    workerOnline' = workerOnline.put(target, not(workerOnline.get(target))),
    opSlot' = opSlot,
    usedAttempts' = usedAttempts,
    issuedList' = issuedList,
    inFlightCalls' = inFlightCalls,
    inFlightReports' = inFlightReports,
    authenticReports' = authenticReports,
    acceptedReports' = acceptedReports,
    lastDelivery' = lastDelivery,
  }

  action step: bool = {
    nondet opId = OPS.oneOf()
    nondet attemptId = ATTEMPTS.oneOf()
    nondet target = WORKERS.oneOf()
    any {
      createOp(opId),
      dispatchDelivered(opId, attemptId, target),
      dispatchFailedBeforeArrival(opId, attemptId, target),
      {
        nondet call = inFlightCalls.oneOf()
        all {
          inFlightCalls.size() > 0,
          workerProduceReport(call),
        }
      },
      {
        nondet call = inFlightCalls.oneOf()
        all {
          inFlightCalls.size() > 0,
          workerFailExecution(call),
        }
      },
      authorityTimeoutOrDisconnect(opId),
      injectMismatchedReport(opId, target),
      {
        nondet rep = inFlightReports.oneOf()
        all {
          inFlightReports.size() > 0,
          authorityReceiveReport(rep),
        }
      },
      authorityClose(opId),
      toggleWorkerOnline(target),
    }
  }

  // 若最近一次签发的 Attempt 尚未被合法 Report 验收, Operation 必须仍处于 AwaitingOp.
  val unresolvedNeverLeavesAwaitingWithoutReport: bool =
    if (issuedList.length() == 0) true
    else {
      val latest = issuedList[issuedList.length() - 1]
      val matched = acceptedReports.exists(r =>
        r.opId == latest.opId and r.attemptId == latest.attemptId and r.worker == latest.worker
      )
      matched or opSlot.get(latest.opId) == AwaitingOp({ attemptId: latest.attemptId, worker: latest.worker })
    }

  // 所有被 Authority 接受的 Report 都必须来自真实 Worker 执行(错配/伪造绝不通过).
  val acceptedReportsAreAuthentic: bool =
    acceptedReports.forall(r => authenticReports.contains(r))

  // Authority 签发的所有 Attempt 身份全局不重复.
  val issuedAttemptsAreUnique: bool =
    issuedList.length() == usedAttempts.size()

  val invariants: bool = and {
    unresolvedNeverLeavesAwaitingWithoutReport,
    acceptedReportsAreAuthentic,
    issuedAttemptsAreUnique,
  }

  val witnessAcceptedAndClosed: bool =
    opSlot.get(1) == ClosedOp and acceptedReports.size() >= 1

  val witnessRetryWithSecondAttempt: bool =
    issuedList.length() == 2

  val witnessFailureKeepsAwaiting: bool =
    lastDelivery == DeliveryNoReport and
      match opSlot.get(1) {
        | AwaitingOp(_) => true
        | _ => false
      }

  val witnessOfflineWorkerReportAccepted: bool =
    acceptedReports.exists(r => not(workerOnline.get(r.worker)))
}
