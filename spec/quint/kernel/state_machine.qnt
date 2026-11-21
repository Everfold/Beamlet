// L1 内核纯转换规则: Operation 三态转换与已知状态重建, 不含模拟状态变量.
module state_machine {
  import primitives.* from "./primitives"

  // Operation 的三态生命周期投影.
  type State[a] =
    | Ready
    | AwaitingReport(a)
    | Closed

  // 状态转换被拒绝的抽象原因.
  type KernelError =
    | AttemptPending
    | OperationClosed
    | NoPendingAttempt
    | OperationMismatch
    | AttemptMismatch

  // 内核动作的返回结果: 签发新授权, 更新状态, 或携带原因拒绝.
  type KernelOutcome[o, a] =
    | Created({ attempt: Attempt[o, a], state: State[a] })
    | Updated(State[a])
    | Rejected(KernelError)

  // 创建 Attempt: 仅 Ready 允许转换, 返回新授权与 AwaitingReport(attemptId).
  pure def createAttempt(
    state: State[a],
    operationId: o,
    attemptId: a
  ): KernelOutcome[o, a] =
    match state {
      | Ready =>
          Created({
            attempt: makeAttempt(operationId, attemptId),
            state: AwaitingReport(attemptId),
          })
      | AwaitingReport(_) => Rejected(AttemptPending)
      | Closed => Rejected(OperationClosed)
    }

  // 接受 Report: 仅 AwaitingReport 允许尝试, 依次校验 Operation 与 Attempt 身份.
  pure def acceptReport(
    state: State[a],
    operationId: o,
    evidence: Report[o, a, r, v]
  ): KernelOutcome[o, a] =
    match state {
      | Ready => Rejected(NoPendingAttempt)
      | Closed => Rejected(OperationClosed)
      | AwaitingReport(pending) =>
          if (evidence.operation_id != operationId) Rejected(OperationMismatch)
          else if (evidence.attempt_id != pending) Rejected(AttemptMismatch)
          else Updated(Ready)
    }

  // 关闭 Operation: Ready 转为 Closed, Closed 幂等保持, AwaitingReport 拒绝.
  pure def close(state: State[a]): KernelOutcome[o, a] =
    match state {
      | Ready => Updated(Closed)
      | Closed => Updated(Closed)
      | AwaitingReport(_) => Rejected(AttemptPending)
    }

  // 从外部已知状态重建 AwaitingReport: 仅校验 Attempt 所属 Operation 匹配, 不签发新授权.
  pure def reconstructAwaitingReport(
    operationId: o,
    attempt: Attempt[o, a]
  ): KernelOutcome[o, a] =
    if (attempt.operation_id != operationId) Rejected(OperationMismatch)
    else Updated(AwaitingReport(attempt.id))

  // 从外部已知状态重建 Closed: 直接返回 Closed, 不重新触发关闭副作用.
  pure def reconstructClosed: KernelOutcome[o, a] =
    Updated(Closed)

  // 应用转换结果; 被拒绝时原状态保持不变.
  pure def nextState(state: State[a], result: KernelOutcome[o, a]): State[a] =
    match result {
      | Created(created) => created.state
      | Updated(newState) => newState
      | Rejected(_) => state
    }
}
