// Selection 零回退、Unresolved 保持等待与迟到合法 Report 验收测试.
module routing_test {
  import routing.* from "./routing"

  // Exact 目标不可路由时即使其他候选可路由也直接裁决 Rejected 且不创建 Operation.
  run exactRejectedWhileOtherRoutableTest =
    init
      .then(toggleWorkerRoutable(1))
      .then(submitExact(1, 1))
      .expect(subOutcome.get(1) == SubRejected and opEntry.get(1) == OpNone and attemptCount.get(1) == 0)

  // 超时裁决 Unresolved 后保持 OpAwaiting; 错配报告不关闭; Worker 离线后迟到真实 Report 仍可关闭.
  run unresolvedAndLateAuthenticReportTest =
    init
      .then(submitExact(1, 1))
      .then(authorityTimeoutOrFailed(1))
      .expect(subOutcome.get(1) == SubUnresolved({ opId: 1, attemptId: 1, worker: 1 })
        and opEntry.get(1) == OpAwaiting({ attemptId: 1, targetWorker: 1 }))
      .then(injectMismatchedReport(1, 1))
      .then(authorityReceiveReport({ reportId: BOGUS_REPORT, opId: 1, attemptId: BOGUS_ATTEMPT, worker: 1 }))
      .expect(opEntry.get(1) == OpAwaiting({ attemptId: 1, targetWorker: 1 }))
      .then(workerReplyReport({ opId: 1, attemptId: 1, worker: 1 }))
      .then(toggleWorkerRoutable(1))
      .then(authorityReceiveReport({ reportId: 1, opId: 1, attemptId: 1, worker: 1 }))
      .expect(opEntry.get(1) == OpClosed({ attemptId: 1, reportId: 1, worker: 1 }))
}
