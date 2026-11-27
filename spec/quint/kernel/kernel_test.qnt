// Operation 内核穷举与场景测试.
module kernel_test {
  import primitives.* from "./primitives"
  import state_machine.* from "./state_machine"
  import kernel.* from "./kernel"

  pure val STATES: Set[State[int]] =
    Set(Ready, Closed).union(ATTEMPT_IDS.map(id => AwaitingReport(id)))

  // 内容相同而身份不同的 Operation 是彼此独立的实例.
  run distinctOperationsTest = assert(
    makeOperation(1, "request") != makeOperation(2, "request")
  )

  // 穷举所有状态与 Attempt 身份组合, 验证创建成功条件与拒绝原因.
  run creationRulesTest = assert(
    STATES.forall(s => ATTEMPT_IDS.forall(id =>
      createAttempt(s, 1, id) == match s {
        | Ready =>
            Created({
              attempt: makeAttempt(1, id),
              state: AwaitingReport(id),
            })
        | AwaitingReport(_) => Rejected(AttemptPending)
        | Closed => Rejected(OperationClosed)
      }
    ))
  )

  // 穷举状态, Operation 与 Attempt 引用组合, 验证校验顺序与接受条件.
  run reportRulesTest = assert(
    STATES.forall(s => OPERATION_IDS.forall(o => ATTEMPT_IDS.forall(a =>
      acceptReport(s, 1, makeReport(1, o, a, "result")) == match s {
        | Ready => Rejected(NoPendingAttempt)
        | Closed => Rejected(OperationClosed)
        | AwaitingReport(pending) =>
            if (o != 1) Rejected(OperationMismatch)
            else if (a != pending) Rejected(AttemptMismatch)
            else Updated(Ready)
      }
    )))
  )

  // 穷举关闭规则, 包含对 Closed 状态的幂等关闭.
  run closeRulesTest = assert(
    STATES.forall(s =>
      close(s) == match s {
        | Ready => Updated(Closed)
        | Closed => Updated(Closed)
        | AwaitingReport(_) => Rejected(AttemptPending)
      }
    )
  )

  // 验证外部状态恢复在匹配与不匹配时的行为.
  run restoreRulesTest = assert(and {
    restoreAwaitingReport(1, makeAttempt(1, 2)) == Updated(AwaitingReport(2)),
    restoreAwaitingReport(1, makeAttempt(2, 2)) == Rejected(OperationMismatch),
    restoreClosed == Updated(Closed),
  })

  // 重试后旧报告不能解除新等待, 关闭后不可再创建 Attempt.
  run lifecycleTest =
    init
      .then(createAction(1))
      .then(acceptAction(1, 1))
      .then(createAction(2))
      .then(acceptAction(1, 1))
      .expect(state == AwaitingReport(2) and outcome == Rejected(AttemptMismatch))
      .then(acceptAction(1, 2))
      .then(closeAction)
      .then(closeAction)
      .then(createAction(3))
      .expect(state == Closed and outcome == Rejected(OperationClosed))
}
