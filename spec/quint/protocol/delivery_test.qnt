// L3 定向交付与失败语义场景测试.
module delivery_test {
  import delivery.* from "./delivery"

  // Exact 目标不可用时即使另一 Worker 就绪也直接拒绝, 绝不回退或创建 Operation.
  run exactRejectionNeverFallsBackTest =
    init
      .then(toggleWorkerReady(1))
      .then(submitExact(1, 1))
      .expect(reqState.get(1) == ReqRejected and opStates.get(1) == OpAbsent)

  // 超时返回 ReqUnresolved 时保持 Operation 为 OpAwaiting, 后续不匹配报告不关闭, 真实报告仍可完成关闭.
  run timeoutUnresolvedAndLateReportTest =
    init
      .then(submitExact(1, 1))
      .then(authorityTimeout(1))
      .expect(reqState.get(1) == ReqUnresolved({ opId: 1, attemptId: 1, worker: 1 })
        and opStates.get(1) == OpAwaiting({ attemptId: 1, targetWorker: 1 }))
      .then(injectBogusReport(1, 1))
      .then(authorityReceiveReport({ reportId: BOGUS_REPORT_ID, opId: 1, attemptId: BOGUS_ATTEMPT_ID, worker: 1 }))
      .expect(opStates.get(1) == OpAwaiting({ attemptId: 1, targetWorker: 1 }))
      .then(toggleWorkerReady(1))
      .then(workerExecute({ opId: 1, attemptId: 1, worker: 1 }))
      .then(authorityReceiveReport({ reportId: 1, opId: 1, attemptId: 1, worker: 1 }))
      .expect(opStates.get(1) == OpClosed({ attemptId: 1, reportId: 1, worker: 1 }))
}
