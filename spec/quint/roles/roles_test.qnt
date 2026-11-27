// Authority 与 Worker 协作场景测试.
module roles_test {
  import roles.* from "./roles"

  // 交付失败(NotDelivered)或等待未知(Unknown)不得推进或关闭 Operation.
  run failedDeliveryKeepsAwaitingTest =
    init
      .then(createOp(1))
      .then(dispatchNotDelivered(1, 1, 1))
      .expect(lastDelivery == DeliveryNotDelivered and opSlot.get(1) == AwaitingOp({ attemptId: 1, worker: 1 }))
      .then(authorityTimeoutOrDisconnect(1))
      .expect(lastDelivery == DeliveryUnknown and opSlot.get(1) == AwaitingOp({ attemptId: 1, worker: 1 }))

  // 错配报告被拒收且保留等待; 真实报告在 Worker 离线后仍被接受, 并支持重签发新 Attempt 后关闭.
  run mismatchRejectedOfflineReportAcceptedAndRetryTest =
    init
      .then(createOp(1))
      .then(dispatchDelivered(1, 1, 1))
      .then(injectMismatchedReport(1, 2))
      .then(authorityReceiveReport({ reportId: BOGUS_REPORT_ID, opId: 1, attemptId: BOGUS_ATTEMPT_ID, worker: 2 }))
      .expect(opSlot.get(1) == AwaitingOp({ attemptId: 1, worker: 1 }))
      .then(workerProduceReport({ opId: 1, attemptId: 1, worker: 1 }))
      .then(toggleWorkerOnline(1))
      .then(authorityReceiveReport({ reportId: 1, opId: 1, attemptId: 1, worker: 1 }))
      .expect(opSlot.get(1) == ReadyOp and lastDelivery == DeliveryReported)
      .then(dispatchDelivered(1, 2, 2))
      .then(workerProduceReport({ opId: 1, attemptId: 2, worker: 2 }))
      .then(authorityReceiveReport({ reportId: 2, opId: 1, attemptId: 2, worker: 2 }))
      .then(authorityClose(1))
      .expect(opSlot.get(1) == ClosedOp and acceptedReports.size() == 2)
}
