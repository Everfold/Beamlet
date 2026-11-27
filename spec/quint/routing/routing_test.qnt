// 会话公告, epoch 隔离与 Selection 路由场景测试.
module routing_test {
  import routing.* from "./routing"

  // 断线重连推进 epoch 后旧租约立即失效, 拒绝旧 epoch 心跳, 新 epoch 心跳后恢复.
  run reconnectInvalidatesLeaseUntilNewEpochTest =
    init
      .then(observePresence(1, 1, 1, ModeReady))
      .then(selectAction(SelectExact1))
      .expect(lastDecision == Selected({ worker: 1, source: 1 }))
      .then(disconnectLink(1))
      .then(selectAction(SelectExact1))
      .expect(lastDecision == RejectedBeforeCreate)
      .then(reconnectLink(1))
      .then(selectAction(SelectExact1))
      .expect(lastDecision == RejectedBeforeCreate)
      .then(observePresence(1, 1, 2, ModeReady))
      .then(selectAction(SelectExact1))
      .expect(lastDecision == RejectedBeforeCreate)
      .then(observePresence(1, 2, 2, ModeReady))
      .then(selectAction(SelectExact1))
      .expect(lastDecision == Selected({ worker: 1, source: 1 }))

  // 重复 sequence 跨来源转发不延长首次到达时间; 乱序旧 sequence 被拒绝.
  run replaySameSequenceDoesNotExtendTtlTest =
    init
      .then(observePresence(1, 1, 1, ModeReady))
      .then(advanceClock)
      .then(observePresence(2, 1, 1, ModeReady))
      .then(advanceClock)
      .then(selectAction(SelectExact1))
      .expect(lastDecision == RejectedBeforeCreate)
      .then(observePresence(1, 1, 2, ModeDraining))
      .then(observePresence(2, 1, 1, ModeReady))
      .expect(match entry {
        | Entry(e) => e.seq == 2 and e.mode == ModeDraining
        | AbsentEntry => false
      })

  // Exact 目标不可用时即使其他候选 Ready 也直接拒绝, 绝不回退.
  run exactNeverFallsBackWhileOtherReadyTest =
    init
      .then(toggleWorker2)
      .then(selectAction(SelectExact1))
      .expect(lastDecision == RejectedBeforeCreate)
      .then(selectAction(SelectAny))
      .expect(lastDecision == Selected({ worker: 2, source: 1 }))
}
