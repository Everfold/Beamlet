use super::*;

/// 验证倒序能力保留空白和空文本, 不先执行 trim.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn reverses_text_without_trimming() {
    for (input, expected) in [
        ("", ""),
        ("abc", "cba"),
        (" Hello 世界\n🙂", "🙂\n界世 olleH "),
        ("\t a \r\n", "\n\r a \t"),
        ("a\0b", "b\0a"),
    ] {
        // 使用既有线协议的标识, 防止实现与声明一起变动时漏检版本变化.
        assert_eq!(
            execute("text.reverse/v1", input).unwrap(),
            expected,
            "输入: {input:?}"
        );
    }
}

/// 验证倒序按 Unicode 标量进行, 而不是按字素簇处理.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn reverses_scalars_not_graphemes() {
    for (input, expected) in [
        ("e\u{301}", "\u{301}e"),
        ("👩\u{200d}💻", "💻\u{200d}👩"),
        ("\u{1f1e8}\u{1f1f3}", "\u{1f1f3}\u{1f1e8}"),
    ] {
        assert_eq!(
            execute(CAPABILITY, input).unwrap(),
            expected,
            "输入: {input:?}"
        );
    }
}

/// 验证倒序处理器拒绝其他能力或版本.
///
/// # 返回值
///
/// `()`; 全部断言通过时正常结束, 断言失败或未预期错误会使测试失败.
#[test]
fn rejects_other_capabilities_and_versions() {
    for capability in [
        "",
        "text.reverse",
        "text.reverse/v2",
        "TEXT.REVERSE/v1",
        "text.reverse/v1 ",
        "text.uppercase/v1",
    ] {
        let error = execute(capability, "abc").unwrap_err();
        assert_eq!(error.to_string(), format!("不支持的能力: {capability}"));
    }
}
