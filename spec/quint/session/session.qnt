// RoleSession 身份、固定能力集、Presence 单调状态与本地单调时钟租约模型.
module session {
  type PresenceMode =
    | ModeAccepting
    | ModeDraining
    | ModeOffline

  type SessionEntry =
    | AbsentEntry
    | Entry({
        caps: int,
        mode: PresenceMode,
        lastSeenAt: int,
      })

  pure val CAPS = Set(1, 2)
  pure val MAX_NOW = 4
  pure val LEASE_TIMEOUT = 2

  pure def modeRank(m: PresenceMode): int =
    match m {
      | ModeAccepting => 1
      | ModeDraining => 2
      | ModeOffline => 3
    }

  pure def isSessionRoutable(entry: SessionEntry, currentTime: int): bool =
    match entry {
      | AbsentEntry => false
      | Entry(e) => e.mode == ModeAccepting and currentTime - e.lastSeenAt < LEASE_TIMEOUT
    }

  var entry: SessionEntry
  var initialCaps: int
  var now: int
  var maxModeRank: int
  var rejectedInvalid: bool

  action init: bool = all {
    entry' = AbsentEntry,
    initialCaps' = 0,
    now' = 0,
    maxModeRank' = 0,
    rejectedInvalid' = false,
  }

  // 会话快照观测规则:
  // 同会话内能力集固定不变, 状态仅按 Accepting -> Draining -> Offline 单调推进并刷新到达时间; 能力不一致或状态回退均被拒绝.
  action observePresence(caps: int, mode: PresenceMode): bool = {
    val rank = modeRank(mode)
    val accepted = match entry {
      | AbsentEntry => true
      | Entry(e) => caps == e.caps and rank >= modeRank(e.mode)
    }
    val nextEntry =
      if (accepted) Entry({ caps: caps, mode: mode, lastSeenAt: now })
      else entry
    val nextInitialCaps = match entry {
      | AbsentEntry => caps
      | Entry(_) => initialCaps
    }
    val nextMaxRank =
      if (accepted and rank > maxModeRank) rank
      else maxModeRank
    all {
      entry' = nextEntry,
      initialCaps' = nextInitialCaps,
      maxModeRank' = nextMaxRank,
      rejectedInvalid' = (rejectedInvalid or not(accepted)),
      now' = now,
    }
  }

  action advanceClock: bool = all {
    now < MAX_NOW,
    now' = now + 1,
    entry' = entry,
    initialCaps' = initialCaps,
    maxModeRank' = maxModeRank,
    rejectedInvalid' = rejectedInvalid,
  }

  action step: bool = any {
    {
      nondet caps = CAPS.oneOf()
      nondet mode = Set(ModeAccepting, ModeDraining, ModeOffline).oneOf()
      observePresence(caps, mode)
    },
    advanceClock,
  }

  // 仅当会话处于 ModeAccepting 且距最近有效快照到达未超出本地租约期限时才可路由.
  val routableRequiresAcceptingAndFreshLease: bool =
    isSessionRoutable(entry, now) iff
      match entry {
        | AbsentEntry => false
        | Entry(e) => e.mode == ModeAccepting and (now - e.lastSeenAt < LEASE_TIMEOUT)
      }

  // 会话能力集固定不变, 状态按 Accepting -> Draining -> Offline 单调推进, 到达时间不超过本地单调时钟.
  val sessionImmutableCapsAndMonotonicMode: bool =
    match entry {
      | AbsentEntry => maxModeRank == 0 and initialCaps == 0
      | Entry(e) => e.caps == initialCaps and modeRank(e.mode) == maxModeRank and e.lastSeenAt <= now
    }

  // 非 Accepting 状态永不可路由.
  val nonAcceptingNeverRoutable: bool =
    match entry {
      | AbsentEntry => not(isSessionRoutable(entry, now))
      | Entry(e) => (e.mode != ModeAccepting) implies not(isSessionRoutable(entry, now))
    }

  val invariants: bool = and {
    routableRequiresAcceptingAndFreshLease,
    sessionImmutableCapsAndMonotonicMode,
    nonAcceptingNeverRoutable,
  }

  val witnessRoutable: bool =
    isSessionRoutable(entry, now)

  val witnessTtlExpires: bool =
    match entry {
      | AbsentEntry => false
      | Entry(e) => e.mode == ModeAccepting and not(isSessionRoutable(entry, now))
    }

  val witnessDrainingStopsRouting: bool =
    match entry {
      | AbsentEntry => false
      | Entry(e) => e.mode == ModeDraining and (now - e.lastSeenAt < LEASE_TIMEOUT) and not(isSessionRoutable(entry, now))
    }

  val witnessInvalidRejected: bool =
    rejectedInvalid
}
