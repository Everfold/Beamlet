// RoleSession、Link 世代与多路径租约场景测试.
module session_test {
  import session.* from "./session"

  // 断线重连推进 epoch 后旧租约立即失效, 拒绝旧 epoch 快照, 直到新 epoch 收到新快照才恢复.
  run reconnectInvalidatesLeaseUntilNewEpochTest =
    init
      .then(observePresence(1, 1, 1, ModeAccepting))
      .expect(isSessionRoutable(links, entry, now))
      .then(disconnectLink(1))
      .expect(not(isSessionRoutable(links, entry, now)))
      .then(reconnectLink(1))
      .expect(not(isSessionRoutable(links, entry, now)))
      .then(observePresence(1, 1, 2, ModeAccepting))
      .expect(not(isSessionRoutable(links, entry, now)))
      .then(observePresence(1, 2, 2, ModeAccepting))
      .expect(isSessionRoutable(links, entry, now))

  // 同 sequence 他路转发不延长 TTL; 乱序旧 sequence 被拒绝.
  run replaySameSequenceDoesNotExtendTtlTest =
    init
      .then(observePresence(1, 1, 1, ModeAccepting))
      .then(advanceClock)
      .then(observePresence(2, 1, 1, ModeAccepting))
      .then(advanceClock)
      .expect(not(isSessionRoutable(links, entry, now)))
      .then(observePresence(1, 1, 2, ModeDraining))
      .then(observePresence(2, 1, 1, ModeAccepting))
      .expect(match entry {
        | Entry(e) => e.seq == 2 and e.mode == ModeDraining
        | AbsentEntry => false
      })

  // 路径 1 Link 断线时路径 2 仍保持可路由, 直至本地 TTL 过期.
  run multiPathFailoverAndTtlExpiryTest =
    init
      .then(observePresence(1, 1, 1, ModeAccepting))
      .then(observePresence(2, 1, 1, ModeAccepting))
      .then(disconnectLink(1))
      .expect(not(isSourceRoutable(links, entry, 1, now)) and isSourceRoutable(links, entry, 2, now))
      .then(advanceClock)
      .then(advanceClock)
      .expect(not(isSessionRoutable(links, entry, now)))
}
