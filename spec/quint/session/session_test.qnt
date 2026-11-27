// RoleSession、Presence 序列号与本地租约场景测试.
module session_test {
  import session.* from "./session"

  // 同 sequence 重放不延长首次到达时间, TTL 到期后退出可路由.
  run replaySameSequenceDoesNotExtendTtlTest =
    init
      .then(observePresence(1, ModeAccepting))
      .expect(isSessionRoutable(entry, now))
      .then(advanceClock)
      .then(observePresence(1, ModeAccepting))
      .expect(replayedSameSeq and isSessionRoutable(entry, now))
      .then(advanceClock)
      .expect(not(isSessionRoutable(entry, now)))

  // 乱序旧 sequence 与同序号冲突状态均被拒绝并保持原视图不变.
  run staleAndConflictingSequenceRejectedTest =
    init
      .then(observePresence(2, ModeDraining))
      .expect(not(isSessionRoutable(entry, now)))
      .then(observePresence(1, ModeAccepting))
      .then(observePresence(2, ModeAccepting))
      .expect(and {
        rejectedStaleOrConflict,
        not(isSessionRoutable(entry, now)),
        entry == Entry({ seq: 2, firstSeenAt: 0, mode: ModeDraining }),
      })

  // 新 sequence 续期本地租约, 收到 Draining 后即使未到期也立即停止路由.
  run freshSequenceRenewsLeaseAndStopsOnDrainingTest =
    init
      .then(observePresence(1, ModeAccepting))
      .then(advanceClock)
      .then(observePresence(2, ModeAccepting))
      .then(advanceClock)
      .expect(isSessionRoutable(entry, now))
      .then(observePresence(3, ModeDraining))
      .expect(not(isSessionRoutable(entry, now)))
}
