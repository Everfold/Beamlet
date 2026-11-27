// RoleSession 身份、Presence 序列号与本地单调时钟租约模型.
module session {
  type PresenceMode =
    | ModeAccepting
    | ModeDraining
    | ModeOffline

  type SessionEntry =
    | AbsentEntry
    | Entry({
        seq: int,
        firstSeenAt: int,
        mode: PresenceMode,
      })

  pure val MAX_SEQ = 3
  pure val MAX_NOW = 4
  pure val LEASE_TIMEOUT = 2

  pure def isSessionRoutable(entry: SessionEntry, currentTime: int): bool =
    match entry {
      | AbsentEntry => false
      | Entry(e) => e.mode == ModeAccepting and currentTime - e.firstSeenAt < LEASE_TIMEOUT
    }

  var entry: SessionEntry
  var now: int
  var maxObservedSeq: int
  var replayedSameSeq: bool
  var rejectedStaleOrConflict: bool

  action init: bool = all {
    entry' = AbsentEntry,
    now' = 0,
    maxObservedSeq' = 0,
    replayedSameSeq' = false,
    rejectedStaleOrConflict' = false,
  }

  // 会话快照观测规则:
  // sequence 递增时以当前本地单调时钟登记或续期; 同序号同状态重放不刷新首次到达时间; 序号回退或同序号内容矛盾则拒绝.
  action observePresence(seq: int, mode: PresenceMode): bool = {
    val isAdvance = match entry {
      | AbsentEntry => seq >= 1
      | Entry(e) => seq > e.seq
    }
    val isSameReplay = match entry {
      | AbsentEntry => false
      | Entry(e) => seq == e.seq and mode == e.mode
    }
    val isRejected = not(isAdvance) and not(isSameReplay)
    val nextEntry =
      if (isAdvance) Entry({ seq: seq, firstSeenAt: now, mode: mode })
      else entry
    val nextMaxSeq =
      if (isAdvance) seq
      else maxObservedSeq
    all {
      entry' = nextEntry,
      maxObservedSeq' = nextMaxSeq,
      replayedSameSeq' = (replayedSameSeq or isSameReplay),
      rejectedStaleOrConflict' = (rejectedStaleOrConflict or isRejected),
      now' = now,
    }
  }

  action advanceClock: bool = all {
    now < MAX_NOW,
    now' = now + 1,
    entry' = entry,
    maxObservedSeq' = maxObservedSeq,
    replayedSameSeq' = replayedSameSeq,
    rejectedStaleOrConflict' = rejectedStaleOrConflict,
  }

  action step: bool = any {
    {
      nondet seq = 1.to(MAX_SEQ).oneOf()
      nondet mode = Set(ModeAccepting, ModeDraining, ModeOffline).oneOf()
      observePresence(seq, mode)
    },
    advanceClock,
  }

  // 仅当会话处于 ModeAccepting 且距最新 sequence 首次到达未超出本地租约期限时才可路由.
  val routableRequiresAcceptingAndFreshLease: bool =
    isSessionRoutable(entry, now) iff
      match entry {
        | AbsentEntry => false
        | Entry(e) => e.mode == ModeAccepting and (now - e.firstSeenAt < LEASE_TIMEOUT)
      }

  // sequence 单调不回退, 且首次到达时间戳不超过当前本地单调时钟.
  val sequenceMonotonicAndFirstSeenValid: bool =
    match entry {
      | AbsentEntry => maxObservedSeq == 0
      | Entry(e) => e.seq == maxObservedSeq and e.firstSeenAt <= now
    }

  // 非 Accepting 状态永不可路由.
  val nonAcceptingNeverRoutable: bool =
    match entry {
      | AbsentEntry => not(isSessionRoutable(entry, now))
      | Entry(e) => (e.mode != ModeAccepting) implies not(isSessionRoutable(entry, now))
    }

  val invariants: bool = and {
    routableRequiresAcceptingAndFreshLease,
    sequenceMonotonicAndFirstSeenValid,
    nonAcceptingNeverRoutable,
  }

  val witnessRoutable: bool =
    isSessionRoutable(entry, now)

  val witnessReplayDoesNotExtendTtl: bool =
    replayedSameSeq and
      match entry {
        | AbsentEntry => false
        | Entry(e) => e.mode == ModeAccepting and now > e.firstSeenAt and not(isSessionRoutable(entry, now))
      }

  val witnessDrainingStopsRouting: bool =
    match entry {
      | AbsentEntry => false
      | Entry(e) => e.mode == ModeDraining and (now - e.firstSeenAt < LEASE_TIMEOUT) and not(isSessionRoutable(entry, now))
    }

  val witnessStaleOrConflictRejected: bool =
    rejectedStaleOrConflict
}
