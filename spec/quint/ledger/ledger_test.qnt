// Ledger 在线执行台账场景测试.
module ledger_test {
  import primitives.* from "../kernel/primitives"
  import state_machine.* from "../kernel/state_machine"
  import ledger.* from "./ledger"

  // 重复创建 Operation 拒绝, 且不影响原 Operation 状态.
  run duplicateCreateOpTest =
    init
      .then(createOpAction(1))
      .expect(lastEvent == LedgerCreatedOp(1))
      .then(createAttemptAction(1, 1))
      .then(createOpAction(1))
      .expect(lastEvent == LedgerErr(OperationExists) and isPendingOperation(ledger.entries.get(1)))

  // 跨 Operation 复用已签发 Attempt 身份必须被拒绝, 且拒绝后新身份仍可合法签发.
  run crossOpAttemptReuseTest =
    init
      .then(createOpAction(1))
      .then(createOpAction(2))
      .then(createAttemptAction(1, 1))
      .expect(lastEvent == LedgerIssuedAttempt({ opId: 1, attemptId: 1 }))
      .then(createAttemptAction(2, 1))
      .expect(lastEvent == LedgerErr(AttemptIdReused) and ledger.entries.get(2) == Managed(Idle))
      .then(createAttemptAction(2, 2))
      .expect(lastEvent == LedgerIssuedAttempt({ opId: 2, attemptId: 2 }))

  // 状态不合法时被拒绝的 Attempt 身份不得污染 usedAttempts.
  run rejectedCreateDoesNotPolluteUsedTest =
    init
      .then(createOpAction(1))
      .then(createOpAction(2))
      .then(createAttemptAction(1, 1))
      .then(createAttemptAction(1, 2))
      .expect(lastEvent == LedgerErr(AttemptPendingErr) and not(ledger.usedAttempts.contains(2)))
      .then(createAttemptAction(2, 2))
      .expect(lastEvent == LedgerIssuedAttempt({ opId: 2, attemptId: 2 }))

  // 错误的 Report 保留等待状态; 接受正确 Report 后允许再次签发新 Attempt 并关闭.
  run mismatchKeepsPendingAndRetryTest =
    init
      .then(createOpAction(1))
      .then(createAttemptAction(1, 1))
      .then(acceptReportAction(1, 2))
      .expect(lastEvent == LedgerErr(ReportMismatch) and pendingAttemptOf(ledger.entries.get(1)) == 1)
      .then(acceptReportAction(1, 1))
      .expect(lastEvent == LedgerAcceptedReport({ opId: 1, attemptId: 1 }))
      .then(createAttemptAction(1, 2))
      .then(acceptReportAction(1, 2))
      .then(closeOpAction(1))
      .then(closeOpAction(1))
      .expect(lastEvent == LedgerClosedOp(1) and ledger.entries.get(1) == Managed(Closed))
}
