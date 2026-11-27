// 会话公告登记, Connection.epoch 世代隔离, Presence.sequence 序校验及 Selection::Any / Exact 路由模型.
module routing {
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

  type RegistryEntry =
    | AbsentEntry
    | Entry({
        seq: int,
        firstSeenAt: int,
        mode: PresenceMode,
        leases: int -> SourceLease,
      })

  type SelectionKind =
    | SelectAny
    | SelectExact1
    | SelectExact2

  type SelectionDecision =
    | DecisionNone
    | RejectedBeforeCreate
    | Selected({ worker: int, source: int })

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

  pure def firstFreshSource(
    linksMap: int -> LinkState,
    entry: RegistryEntry,
    currentTime: int
  ): int =
    match entry {
      | AbsentEntry => 0
      | Entry(e) =>
          if (e.mode != ModeReady) 0
          else if (leaseIsFresh(linksMap.get(1), e.leases.get(1), currentTime)) 1
          else if (leaseIsFresh(linksMap.get(2), e.leases.get(2), currentTime)) 2
          else 0
    }

  pure def sourceSeqMatchesOrNewer(lease: SourceLease, seq: int): bool =
    match lease {
      | NoLease => false
      | ActiveLease(l) => seq <= l.seq
    }

  // 会话快照观测规则:
  // sequence 单调递减被拒; 同一 sequence 仅在尚未登记的来源上补充租约, 且不延长首次到达时间.
  pure def canAcceptPresence(
    linksMap: int -> LinkState,
    entry: RegistryEntry,
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
            else if (seq == e.seq and (mode != e.mode or sourceSeqMatchesOrNewer(e.leases.get(src), seq))) false
            else true
      }
  }

  pure def applyPresence(
    entry: RegistryEntry,
    src: int,
    observedEpoch: int,
    seq: int,
    mode: PresenceMode,
    currentTime: int
  ): RegistryEntry =
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

  pure def evalSelection(
    linksMap: int -> LinkState,
    entry: RegistryEntry,
    worker2Ready: bool,
    currentTime: int,
    sel: SelectionKind
  ): SelectionDecision = {
    val s1 = firstFreshSource(linksMap, entry, currentTime)
    match sel {
      | SelectExact1 =>
          if (s1 != 0) Selected({ worker: 1, source: s1 })
          else RejectedBeforeCreate
      | SelectExact2 =>
          if (worker2Ready) Selected({ worker: 2, source: 1 })
          else RejectedBeforeCreate
      | SelectAny =>
          if (s1 != 0) Selected({ worker: 1, source: s1 })
          else if (worker2Ready) Selected({ worker: 2, source: 1 })
          else RejectedBeforeCreate
    }
  }

  var links: int -> LinkState
  var entry: RegistryEntry
  var worker2Ready: bool
  var now: int
  var lastSelection: SelectionKind
  var lastDecision: SelectionDecision
  var maxObservedSeq: int
  var replayedSameSeq: bool

  action init: bool = all {
    links' = SOURCES.mapBy(_ => { epoch: 1, connected: true }),
    entry' = AbsentEntry,
    worker2Ready' = false,
    now' = 0,
    lastSelection' = SelectAny,
    lastDecision' = DecisionNone,
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
      worker2Ready' = worker2Ready,
      now' = now,
      lastSelection' = lastSelection,
      lastDecision' = DecisionNone,
    }
  }

  action disconnectLink(src: int): bool = {
    val link = links.get(src)
    all {
      link.connected,
      links' = links.put(src, { ...link, connected: false }),
      entry' = entry,
      worker2Ready' = worker2Ready,
      now' = now,
      lastSelection' = lastSelection,
      lastDecision' = DecisionNone,
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
      worker2Ready' = worker2Ready,
      now' = now,
      lastSelection' = lastSelection,
      lastDecision' = DecisionNone,
      maxObservedSeq' = maxObservedSeq,
      replayedSameSeq' = replayedSameSeq,
    }
  }

  action toggleWorker2: bool = all {
    worker2Ready' = not(worker2Ready),
    links' = links,
    entry' = entry,
    now' = now,
    lastSelection' = lastSelection,
    lastDecision' = DecisionNone,
    maxObservedSeq' = maxObservedSeq,
    replayedSameSeq' = replayedSameSeq,
  }

  action advanceClock: bool = all {
    now < MAX_NOW,
    now' = now + 1,
    links' = links,
    entry' = entry,
    worker2Ready' = worker2Ready,
    lastSelection' = lastSelection,
    lastDecision' = DecisionNone,
    maxObservedSeq' = maxObservedSeq,
    replayedSameSeq' = replayedSameSeq,
  }

  action selectAction(sel: SelectionKind): bool = all {
    lastSelection' = sel,
    lastDecision' = evalSelection(links, entry, worker2Ready, now, sel),
    links' = links,
    entry' = entry,
    worker2Ready' = worker2Ready,
    now' = now,
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
    toggleWorker2,
    advanceClock,
    {
      nondet sel = Set(SelectAny, SelectExact1, SelectExact2).oneOf()
      selectAction(sel)
    },
  }

  // 被选中的 Worker 1 必须处于 ModeReady, 且出口链路在线, 世代一致, 未过期.
  val selectedWorker1IsFreshAndCurrentEpoch: bool =
    match lastDecision {
      | Selected(s) =>
          if (s.worker != 1) true
          else match entry {
            | AbsentEntry => false
            | Entry(e) =>
                e.mode == ModeReady and leaseIsFresh(links.get(s.source), e.leases.get(s.source), now)
          }
      | _ => true
    }

  // Presence.sequence 单调不回退, 且相同 sequence 在他源转发不延长首次到达时间戳.
  val sequenceMonotonicAndReplayNeverExtendsTtl: bool =
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

  // Selection::Exact 绝不回退到其他 Worker.
  val exactSelectionNeverFallsBack: bool =
    match lastDecision {
      | Selected(s) =>
          and {
            (lastSelection == SelectExact1) implies (s.worker == 1),
            (lastSelection == SelectExact2) implies (s.worker == 2),
          }
      | _ => true
    }

  // 任一来源断线或推进 epoch 不破坏另一在线且未过期来源的可用性.
  val multiSourceFaultIsolation: bool =
    match entry {
      | AbsentEntry => true
      | Entry(e) =>
          SOURCES.forall(src =>
            (e.mode == ModeReady and leaseIsFresh(links.get(src), e.leases.get(src), now))
              implies firstFreshSource(links, entry, now) != 0
          )
    }

  val invariants: bool = and {
    selectedWorker1IsFreshAndCurrentEpoch,
    sequenceMonotonicAndReplayNeverExtendsTtl,
    exactSelectionNeverFallsBack,
    multiSourceFaultIsolation,
  }

  val witnessSelected: bool =
    match lastDecision {
      | Selected(_) => true
      | _ => false
    }

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
        | Entry(e) => now > e.firstSeenAt and firstFreshSource(links, entry, now) == 0
      }

  val witnessExactRejectedWhileOtherReady: bool =
    lastDecision == RejectedBeforeCreate and lastSelection == SelectExact1 and worker2Ready
}
