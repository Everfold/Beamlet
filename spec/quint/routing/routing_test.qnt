// Target 零回退、多 Attempt 重试派发与锁定来源验收测试.
module routing_test {
  import routing.* from "./routing"

  // Exact 目标不可路由时即使其他候选可路由也直接拒绝派发, 不签发 Attempt 且保持 OpIdle.
  run exactRejectedWhileOtherRoutableTest =
    init
      .then(createOperation)
      .then(toggleWorkerRoutable(1))
      .then(dispatchExact(1, 1))
      .expect(opState == OpIdle and usedAttempts == Set() and preMintRejected)

  // 同一 Operation 连续执行两次 Attempt(跨不同 Worker 选路)后再显式关闭.
  run multiAttemptRetryAcrossWorkersThenCloseTest =
    init
      .then(createOperation)
      .then(dispatchExact(1, 1))
      .then(workerReplyReport({ attemptId: 1, worker: 1 }))
      .then(authorityReceiveReply({ worker: 1, attemptId: 1, kind: ReplyReport(1) }))
      .expect(opState == OpIdle and acceptedCount == 1)
      .then(dispatchExact(2, 2))
      .then(workerReplyReport({ attemptId: 2, worker: 2 }))
      .then(authorityReceiveReply({ worker: 2, attemptId: 2, kind: ReplyReport(2) }))
      .expect(opState == OpIdle and acceptedCount == 2)
      .then(closeOperation)
      .expect(opState == OpClosed and usedAttempts == Set(1, 2))

  // 失败通知与伪造来源回复均保持 OpAwaiting; 目标离线后迟到的真实锁定 Report 仍可验收回到 OpIdle.
  run failedAndSpoofedRepliesKeepAwaitingUntilAuthenticLateReportTest =
    init
      .then(createOperation)
      .then(dispatchExact(1, 1))
      .then(injectInvalidReply(true))
      .then(authorityReceiveReply({ worker: 2, attemptId: 1, kind: ReplyReport(BOGUS_REPORT) }))
      .expect(opState == OpAwaiting({ attemptId: 1, lockedWorker: 1, target: TargetExactKind(1) }))
      .then(authorityTimeout)
      .expect(unresolvedWaitEnded and opState == OpAwaiting({ attemptId: 1, lockedWorker: 1, target: TargetExactKind(1) }))
      .then(workerReplyReport({ attemptId: 1, worker: 1 }))
      .then(toggleWorkerRoutable(1))
      .then(authorityReceiveReply({ worker: 1, attemptId: 1, kind: ReplyReport(1) }))
      .expect(opState == OpIdle and acceptedCount == 1)
}
