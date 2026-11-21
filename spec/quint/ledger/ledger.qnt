// L2 在线台账: 管理多个 Operation 生命周期并强制 Attempt 身份全局不重复.
// 不依赖 L3 角色, 传输或存储细节.
module ledger {
  import primitives.* from "../kernel/primitives"
  import state_machine.* from "../kernel/state_machine"

  // 台账中 Operation 槽位状态.
  type LedgerSlot =
    | Uncreated
    | Managed(State[int])

  // 台账操作拒绝原因 (对应 ledger.proto 中的 LedgerErrorReason).
  type LedgerError =
    | OperationExists
    | OperationNotFound
    | AttemptPendingErr
    | NoPendingAttemptErr
    | OperationClosedErr
    | AttemptIdReused
    | ReportMismatch

  // 聚合在线台账内部状态.
  type LedgerState = {
    entries: int -> LedgerSlot,
    usedAttempts: Set[int],
  }

  // 台账动作观测结果.
  type LedgerOutcome =
    | LedgerInit
    | LedgerCreatedOp(int)
    | LedgerIssuedAttempt({ opId: int, attemptId: int })
    | LedgerAcceptedReport({ opId: int, attemptId: int })
    | LedgerClosedOp(int)
    | LedgerErr(LedgerError)

  type LedgerResult = {
    ledger: LedgerState,
    outcome: LedgerOutcome,
  }

  pure val OPERATION_IDS = Set(1, 2)
  pure val ATTEMPT_IDS = Set(1, 2, 3)

  // 创建新的 Ready Operation; 若身份已存在则拒绝且不覆盖原记录.
  pure def ledgerCreateOperation(state: LedgerState, opId: int): LedgerResult =
    match state.entries.get(opId) {
      | Uncreated =>
          {
            ledger: {
              ...state,
              entries: state.entries.put(opId, Managed(Ready)),
            },
            outcome: LedgerCreatedOp(opId),
          }
      | Managed(_) =>
          {
            ledger: state,
            outcome: LedgerErr(OperationExists),
          }
    }

  // 为 Ready Operation 签发未曾使用的 Attempt; 任何拒绝都不污染 usedAttempts.
  pure def ledgerCreateAttempt(
    state: LedgerState,
    opId: int,
    attemptId: int
  ): LedgerResult =
    match state.entries.get(opId) {
      | Uncreated => { ledger: state, outcome: LedgerErr(OperationNotFound) }
      | Managed(opState) =>
          match opState {
            | AwaitingReport(_) =>
                { ledger: state, outcome: LedgerErr(AttemptPendingErr) }
            | Closed =>
                { ledger: state, outcome: LedgerErr(OperationClosedErr) }
            | Ready =>
                if (state.usedAttempts.contains(attemptId))
                  { ledger: state, outcome: LedgerErr(AttemptIdReused) }
                else
                  match createAttempt(opState, opId, attemptId) {
                    | Created(created) =>
                        {
                          ledger: {
                            entries: state.entries.put(opId, Managed(created.state)),
                            usedAttempts: state.usedAttempts.union(Set(attemptId)),
                          },
                          outcome: LedgerIssuedAttempt({ opId: opId, attemptId: attemptId }),
                        }
                    | Updated(_) => { ledger: state, outcome: LedgerErr(AttemptPendingErr) }
                    | Rejected(_) => { ledger: state, outcome: LedgerErr(AttemptPendingErr) }
                  }
          }
    }

  // 按 Report.operation_id 查找并校验; 不匹配时保留原 AwaitingReport 状态.
  pure def ledgerAcceptReport(
    state: LedgerState,
    evidence: Report[int, int, int, str]
  ): LedgerResult =
    match state.entries.get(evidence.operation_id) {
      | Uncreated => { ledger: state, outcome: LedgerErr(OperationNotFound) }
      | Managed(opState) =>
          match opState {
            | Ready =>
                { ledger: state, outcome: LedgerErr(NoPendingAttemptErr) }
            | Closed =>
                { ledger: state, outcome: LedgerErr(OperationClosedErr) }
            | AwaitingReport(pendingAttempt) =>
                match acceptReport(opState, evidence.operation_id, evidence) {
                  | Updated(newState) =>
                      {
                        ledger: {
                          ...state,
                          entries: state.entries.put(evidence.operation_id, Managed(newState)),
                        },
                        outcome: LedgerAcceptedReport({
                          opId: evidence.operation_id,
                          attemptId: pendingAttempt,
                        }),
                      }
                  | Rejected(_) =>
                      { ledger: state, outcome: LedgerErr(ReportMismatch) }
                  | Created(_) =>
                      { ledger: state, outcome: LedgerErr(ReportMismatch) }
                }
          }
    }

  // 关闭 Ready 或幂等关闭 Closed Operation; 等待中时拒绝.
  pure def ledgerClose(state: LedgerState, opId: int): LedgerResult =
    match state.entries.get(opId) {
      | Uncreated => { ledger: state, outcome: LedgerErr(OperationNotFound) }
      | Managed(opState) =>
          match close(opState) {
            | Updated(newState) =>
                {
                  ledger: {
                    ...state,
                    entries: state.entries.put(opId, Managed(newState)),
                  },
                  outcome: LedgerClosedOp(opId),
                }
            | Rejected(_) =>
                { ledger: state, outcome: LedgerErr(AttemptPendingErr) }
            | Created(_) =>
                { ledger: state, outcome: LedgerErr(AttemptPendingErr) }
          }
    }

  // --- 状态机与辅助历史变量 (用于安全不变量) ---

  var ledger: LedgerState
  var lastOutcome: LedgerOutcome
  var issuedLog: List[{ opId: int, attemptId: int }]
  var everClosed: Set[int]
  var acceptedOps: Set[int]
  var retriedOps: Set[int]

  action init: bool = all {
    ledger' = {
      entries: OPERATION_IDS.mapBy(_ => Uncreated),
      usedAttempts: Set(),
    },
    lastOutcome' = LedgerInit,
    issuedLog' = List(),
    everClosed' = Set(),
    acceptedOps' = Set(),
    retriedOps' = Set(),
  }

  action applyLedger(res: LedgerResult): bool = all {
    ledger' = res.ledger,
    lastOutcome' = res.outcome,
    issuedLog' = match res.outcome {
      | LedgerIssuedAttempt(entry) => issuedLog.append(entry)
      | _ => issuedLog
    },
    everClosed' = match res.outcome {
      | LedgerClosedOp(opId) => everClosed.union(Set(opId))
      | _ => everClosed
    },
    acceptedOps' = match res.outcome {
      | LedgerAcceptedReport(ev) => acceptedOps.union(Set(ev.opId))
      | _ => acceptedOps
    },
    retriedOps' = match res.outcome {
      | LedgerIssuedAttempt(entry) =>
          if (acceptedOps.contains(entry.opId)) retriedOps.union(Set(entry.opId))
          else retriedOps
      | _ => retriedOps
    },
  }

  action createOpAction(opId: int): bool =
    applyLedger(ledgerCreateOperation(ledger, opId))

  action createAttemptAction(opId: int, attemptId: int): bool =
    applyLedger(ledgerCreateAttempt(ledger, opId, attemptId))

  action acceptReportAction(opId: int, attemptId: int): bool =
    applyLedger(ledgerAcceptReport(ledger, makeReport(1, opId, attemptId, "ok")))

  action closeOpAction(opId: int): bool =
    applyLedger(ledgerClose(ledger, opId))

  action step: bool = {
    nondet opId = OPERATION_IDS.oneOf()
    nondet attemptId = ATTEMPT_IDS.oneOf()
    any {
      createOpAction(opId),
      createAttemptAction(opId, attemptId),
      acceptReportAction(opId, attemptId),
      closeOpAction(opId),
    }
  }

  // --- 安全不变量 (对应 03-ledger.md 中的 INV-LDG-*) ---

  pure def isPendingOperation(slot: LedgerSlot): bool =
    match slot {
      | Managed(s) =>
          match s {
            | AwaitingReport(_) => true
            | _ => false
          }
      | Uncreated => false
    }

  pure def pendingAttemptOf(slot: LedgerSlot): int =
    match slot {
      | Managed(s) =>
          match s {
            | AwaitingReport(a) => a
            | _ => -1
          }
      | Uncreated => -1
    }

  // INV-LDG-01: 已关闭的 Operation 保持 Closed, 永不重开或被覆盖.
  val inv_ldg_01_closed_monotonic: bool =
    everClosed.forall(opId => ledger.entries.get(opId) == Managed(Closed))

  // INV-LDG-02: 全局签发日志中的 Attempt 身份彼此唯一, 且与 usedAttempts 完全一致.
  val inv_ldg_02_attempt_id_globally_unique: bool = {
    val issuedSet = issuedLog.foldl(Set(), (acc, entry) => acc.union(Set(entry.attemptId)))
    and {
      issuedSet.size() == issuedLog.length(),
      issuedSet == ledger.usedAttempts,
    }
  }

  // INV-LDG-03: 被拒绝的创建尝试绝不污染 usedAttempts.
  val inv_ldg_03_rejected_does_not_pollute_used: bool =
    ledger.usedAttempts.size() == issuedLog.length()

  // INV-LDG-04: 所有等待中 Operation 持有的 Attempt 均已记录于 usedAttempts, 且互不相同.
  val inv_ldg_04_pending_attempt_is_used: bool = and {
    OPERATION_IDS.forall(opId => {
      val slot = ledger.entries.get(opId)
      not(isPendingOperation(slot)) or ledger.usedAttempts.contains(pendingAttemptOf(slot))
    }),
    OPERATION_IDS.forall(o1 => OPERATION_IDS.forall(o2 =>
      (o1 == o2 or not(isPendingOperation(ledger.entries.get(o1))) or not(isPendingOperation(ledger.entries.get(o2))))
        or pendingAttemptOf(ledger.entries.get(o1)) != pendingAttemptOf(ledger.entries.get(o2))
    )),
  }

  val invariants: bool = and {
    inv_ldg_01_closed_monotonic,
    inv_ldg_02_attempt_id_globally_unique,
    inv_ldg_03_rejected_does_not_pollute_used,
    inv_ldg_04_pending_attempt_is_used,
  }

  // --- 可达性见证 ---

  val witnessTwoOpsActive: bool =
    OPERATION_IDS.forall(opId => isPendingOperation(ledger.entries.get(opId)))

  val witnessAttemptReuseRejected: bool =
    lastOutcome == LedgerErr(AttemptIdReused)

  val witnessRetryWithFreshAttempt: bool =
    retriedOps.size() > 0
}
