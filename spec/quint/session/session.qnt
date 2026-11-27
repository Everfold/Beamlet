// 会话身份、Presence 序列号与多路径 Connection 世代租约模型.
module session {
  type PresenceMode =
    | ModeReady
    | ModeDraining
    | ModeOffline

  type LinkState = {
    epoch: int,
    connected: bool,
  }

  type SourceLease =
    | NoLease
    | ActiveLease({ epoch: int, receivedAt: int, seq: int })

  type SessionEntry =
    | AbsentEntry
    | Entry({
        seq: int,
        firstSeenAt: int,
        mode: PresenceMode,
        leases: int -> SourceLease,
      })

  pure val SOURCES = Set(1, 2)
  pure val MAX_SEQ = 2
  pure val MAX_EPOCHS = 2
  pure val MAX_NOW = 3
  pure val LEASE_TIMEOUT = 2

  pure def leaseIsFresh(link: LinkState, lease: SourceLease, currentTime: int): bool =
    match lease {
      | NoLease => false
      | ActiveLease(l) => and {
          link.connected,
          l.epoch == link.epoch,
          currentTime - l.receivedAt < LEASE_TIMEOUT,
        }
    }

  pure def isSourceRoutable(
    linksMap: int -> LinkState,
    entry: SessionEntry,
    src: int,
    currentTime: int
  ): bool =
    match entry {
      | AbsentEntry => false
      | Entry(e) =>
          e.mode == ModeReady and leaseIsFresh(linksMap.get(src), e.leases.get(src), currentTime)
    }

  pure def isSessionRoutable(
    linksMap: int -> LinkState,
    entry: SessionEntry,
    currentTime: int
  ): bool =
    SOURCES.exists(src => isSourceRoutable(linksMap, entry, src, currentTime))

  pure def sourceSeqAtLeast(lease: SourceLease, seq: int): bool =
    match lease {
      | NoLease => false
      | ActiveLease(l) => seq <= l.seq
    }

  // 会话快照观测规则:
  // sequence 递减或同序号内容矛盾均被拒绝; 同序号经他路转发仅补充路径可用性, 不延长首次到达时间.
  pure def canAcceptPresence(
    linksMap: int -> LinkState,
    entry: SessionEntry,
    src: int,
    observedEpoch: int,
    seq: int,
    mode: PresenceMode
  ): bool = {
    val link = linksMap.get(src)
    if (not(link.connected) or link.epoch != observedEpoch or seq < 1) false
    else
      match entry {
        | AbsentEntry => true
        | Entry(e) =>
            if (seq < e.seq) false
            else if (seq == e.seq and (mode != e.mode or sourceSeqAtLeast(e.leases.get(src), seq))) false
            else true
      }
  }

  pure def applyPresence(
    entry: SessionEntry,
    src: int,
    observedEpoch: int,
    seq: int,
    mode: PresenceMode,
    currentTime: int
  ): SessionEntry =
    match entry {
      | AbsentEntry =>
          Entry({
            seq: seq,
            firstSeenAt: currentTime,
            mode: mode,
            leases: SOURCES.mapBy(s =>
              if (s == src) ActiveLease({ epoch: observedEpoch, receivedAt: currentTime, seq: seq })
              else NoLease
            ),
          })
      | Entry(e) => {
          val nextFirstSeen = if (seq > e.seq) currentTime else e.firstSeenAt
          Entry({
            seq: seq,
            firstSeenAt: nextFirstSeen,
            mode: mode,
            leases: e.leases.put(src, ActiveLease({ epoch: observedEpoch, receivedAt: nextFirstSeen, seq: seq })),
          })
        }
    }

  var links: int -> LinkState
  var entry: SessionEntry
  var now: int
  var maxObservedSeq: int
  var replayedSameSeq: bool

  action init: bool = all {
    links' = SOURCES.mapBy(_ => { epoch: 1, connected: true }),
    entry' = AbsentEntry,
    now' = 0,
    maxObservedSeq' = 0,
    replayedSameSeq' = false,
  }

  action observePresence(
    src: int,
    observedEpoch: int,
    seq: int,
    mode: PresenceMode
  ): bool = {
    val accepted = canAcceptPresence(links, entry, src, observedEpoch, seq, mode)
    val sameSeqNewSource = match entry {
      | AbsentEntry => false
      | Entry(e) => accepted and seq == e.seq
    }
    val nextEntry =
      if (accepted) applyPresence(entry, src, observedEpoch, seq, mode, now)
      else entry
    val nextMaxSeq =
      if (accepted and seq > maxObservedSeq) seq
      else maxObservedSeq
    all {
      entry' = nextEntry,
      maxObservedSeq' = nextMaxSeq,
      replayedSameSeq' = (replayedSameSeq or sameSeqNewSource),
      links' = links,
      now' = now,
    }
  }

  action disconnectLink(src: int): bool = {
    val link = links.get(src)
    all {
      link.connected,
      links' = links.put(src, { ...link, connected: false }),
      entry' = entry,
      now' = now,
      maxObservedSeq' = maxObservedSeq,
      replayedSameSeq' = replayedSameSeq,
    }
  }

  action reconnectLink(src: int): bool = {
    val link = links.get(src)
    all {
      not(link.connected),
      link.epoch < MAX_EPOCHS,
      links' = links.put(src, { epoch: link.epoch + 1, connected: true }),
      entry' = entry,
      now' = now,
      maxObservedSeq' = maxObservedSeq,
      replayedSameSeq' = replayedSameSeq,
    }
  }

  action advanceClock: bool = all {
    now < MAX_NOW,
    now' = now + 1,
    links' = links,
    entry' = entry,
    maxObservedSeq' = maxObservedSeq,
    replayedSameSeq' = replayedSameSeq,
  }

  action step: bool = any {
    {
      nondet src = SOURCES.oneOf()
      nondet ep = 1.to(MAX_EPOCHS).oneOf()
      nondet seq = 1.to(MAX_SEQ).oneOf()
      nondet mode = Set(ModeReady, ModeDraining, ModeOffline).oneOf()
      observePresence(src, ep, seq, mode)
    },
    {
      nondet src = SOURCES.oneOf()
      disconnectLink(src)
    },
    {
      nondet src = SOURCES.oneOf()
      reconnectLink(src)
    },
    advanceClock,
  }

  // 仅当会话处于 ModeReady 且路径链路在线、世代一致、未超时, 该路径才可路由.
  val routableRequiresReadyAndFreshEpoch: bool =
    SOURCES.forall(src =>
      isSourceRoutable(links, entry, src, now) iff
        match entry {
          | AbsentEntry => false
          | Entry(e) => e.mode == ModeReady and leaseIsFresh(links.get(src), e.leases.get(src), now)
        }
    )

  // sequence 单调不回退, 且相同 sequence 转发不延长首次到达时间戳.
  val sequenceMonotonicAndReplayDoesNotExtendTtl: bool =
    match entry {
      | AbsentEntry => maxObservedSeq == 0
      | Entry(e) => and {
          e.seq == maxObservedSeq,
          SOURCES.forall(src =>
            match e.leases.get(src) {
              | NoLease => true
              | ActiveLease(l) => l.receivedAt == e.firstSeenAt or l.seq < e.seq
            }
          ),
        }
    }

  // 一条路径断线或推进 epoch 不破坏另一条在线且新鲜路径的可路由性.
  val multiPathFaultIsolation: bool =
    match entry {
      | AbsentEntry => true
      | Entry(e) =>
          SOURCES.forall(src =>
            (e.mode == ModeReady and leaseIsFresh(links.get(src), e.leases.get(src), now))
              implies isSessionRoutable(links, entry, now)
          )
    }

  val invariants: bool = and {
    routableRequiresReadyAndFreshEpoch,
    sequenceMonotonicAndReplayDoesNotExtendTtl,
    multiPathFaultIsolation,
  }

  val witnessRoutable: bool =
    isSessionRoutable(links, entry, now)

  val witnessReconnectInvalidatesOldEpoch: bool =
    SOURCES.exists(s =>
      links.get(s).epoch == 2 and
        match entry {
          | AbsentEntry => false
          | Entry(e) =>
              match e.leases.get(s) {
                | NoLease => false
                | ActiveLease(l) => l.epoch == 1
              }
        }
    )

  val witnessReplayDoesNotExtendTtl: bool =
    replayedSameSeq and
      match entry {
        | AbsentEntry => false
        | Entry(e) => now > e.firstSeenAt and not(isSessionRoutable(links, entry, now))
      }

  val witnessMultiPathFailover: bool =
    not(links.get(1).connected) and isSourceRoutable(links, entry, 2, now)
}
