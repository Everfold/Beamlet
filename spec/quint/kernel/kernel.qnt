// Operation 内核有界模型探索: 验证生命周期不变量与可达性见证.
module kernel {
  import primitives.* from "./primitives"
  import state_machine.* from "./state_machine"

  pure val CURRENT_OP = makeOperation(1, "request")
  pure val OPERATION_IDS = Set(1, 2)
  pure val ATTEMPT_IDS = Set(1, 2, 3)

  type ReportInput =
    | NoReport
    | SubmittedReport({ operation_id: int, attempt_id: int })

  var state: State[int]
  var previous: State[int]
  var outcome: KernelOutcome[int, int]
  var used: Set[int]
  var reportInput: ReportInput

  action init: bool = all {
    state' = Idle,
    previous' = Idle,
    outcome' = Updated(Idle),
    used' = Set(),
    reportInput' = NoReport,
  }

  action applyOutcome(
    result: KernelOutcome[int, int],
    input: ReportInput
  ): bool = all {
    previous' = state,
    state' = nextState(state, result),
    outcome' = result,
    reportInput' = input,
    used' = match result {
      | Created(created) => used.union(Set(created.attempt.id))
      | _ => used
    },
  }

  // Idle 时外层提供未使用 Attempt 身份; 非 Idle 时模拟非法触发以验证拒绝语义.
  action createAction(attemptId: int): bool = all {
    state != Idle or not(used.contains(attemptId)),
    applyOutcome(createAttempt(state, CURRENT_OP.id, attemptId), NoReport),
  }

  action acceptAction(opId: int, attemptId: int): bool =
    applyOutcome(
      acceptReport(
        state,
        CURRENT_OP.id,
        makeReport(1, opId, attemptId, "result")
      ),
      SubmittedReport({ operation_id: opId, attempt_id: attemptId })
    )

  action closeAction: bool =
    applyOutcome(close(state), NoReport)

  action step: bool = {
    nondet attemptId = ATTEMPT_IDS.oneOf()
    nondet opId = OPERATION_IDS.oneOf()
    any {
      createAction(attemptId),
      acceptAction(opId, attemptId),
      closeAction,
    }
  }

  // 被拒绝的动作不改变状态.
  val rejectionKeepsState: bool =
    match outcome {
      | Rejected(_) => state == previous
      | _ => true
    }

  // 进入 Closed 后不再回到 Idle 或 AwaitingReport.
  val closedStaysClosed: bool =
    previous != Closed or state == Closed

  // CreateAttempt 仅在 Idle 成功, 且授权与等待状态一致.
  val creationIsConsistent: bool =
    match outcome {
      | Created(created) => and {
          previous == Idle,
          created.attempt.operation_id == CURRENT_OP.id,
          state == AwaitingReport(created.attempt.id),
        }
      | _ => true
    }

  // AcceptReport 仅在 AwaitingReport 且双身份均匹配时成功并回到 Idle.
  val reportAcceptanceIsValid: bool =
    match reportInput {
      | NoReport => true
      | SubmittedReport(ev) =>
          match outcome {
            | Rejected(_) => true
            | Created(_) => false
            | Updated(newState) =>
                match previous {
                  | AwaitingReport(pending) => and {
                      ev.operation_id == CURRENT_OP.id,
                      ev.attempt_id == pending,
                      newState == Idle,
                      state == Idle,
                    }
                  | _ => false
                }
          }
    }

  val invariants: bool = and {
    rejectionKeepsState,
    closedStaysClosed,
    creationIsConsistent,
    reportAcceptanceIsValid,
  }

  val witnessAttemptCreated: bool =
    match outcome {
      | Created(_) => true
      | _ => false
    }

  val witnessReportAccepted: bool =
    match reportInput {
      | NoReport => false
      | SubmittedReport(_) =>
          match outcome {
            | Updated(newState) => newState == Idle
            | _ => false
          }
    }

  val witnessClosed: bool = state == Closed

  val witnessRejectedMismatch: bool =
    match outcome {
      | Rejected(err) => err == OperationMismatch or err == AttemptMismatch
      | _ => false
    }
}
