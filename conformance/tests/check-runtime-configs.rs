use std::fs;
use std::path::{Path, PathBuf};

fn fail(message: impl AsRef<str>) -> ! {
    eprintln!("runtime-config conformance failed: {}", message.as_ref());
    std::process::exit(1);
}

fn require(condition: bool, message: impl AsRef<str>) {
    if !condition {
        fail(message);
    }
}

fn root() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|error| fail(format!("cannot resolve cwd: {error}")))
}

fn read_real_file(root: &Path, name: &str) -> String {
    let path = root.join(name);
    let meta = fs::symlink_metadata(&path)
        .unwrap_or_else(|error| fail(format!("missing {name}: {error}")));
    require(meta.is_file(), format!("{name} must be a regular file"));
    require(!meta.file_type().is_symlink(), format!("{name} must not be a symlink"));
    fs::read_to_string(path).unwrap_or_else(|error| fail(format!("cannot read {name}: {error}")))
}

fn toml_root(raw: &str) -> &str {
    raw.find("\n[").map_or(raw, |idx| &raw[..idx])
}

fn toml_section<'a>(raw: &'a str, header: &str) -> &'a str {
    let mut offset = 0usize;
    let mut start = None;
    for line in raw.split_inclusive('\n') {
        let trimmed = line.trim();
        if let Some(section_start) = start {
            if trimmed.starts_with('[') {
                return &raw[section_start..offset];
            }
        } else if trimmed == header {
            start = Some(offset + line.len());
        }
        offset += line.len();
    }
    let section_start = start.unwrap_or_else(|| fail(format!("missing TOML section {header}")));
    &raw[section_start..]
}

fn toml_raw_field<'a>(section: &'a str, key: &str) -> &'a str {
    for line in section.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((lhs, rhs)) = line.split_once('=') {
            if lhs.trim() == key {
                return rhs.trim();
            }
        }
    }
    fail(format!("missing TOML field {key}"));
}

fn toml_string<'a>(section: &'a str, key: &str) -> &'a str {
    let value = toml_raw_field(section, key);
    value
        .strip_prefix('"')
        .and_then(|inner| inner.strip_suffix('"'))
        .unwrap_or_else(|| fail(format!("TOML field {key} must be a quoted string")))
}

fn toml_int(section: &str, key: &str) -> i64 {
    toml_raw_field(section, key)
        .parse::<i64>()
        .unwrap_or_else(|error| fail(format!("TOML field {key} must be integer: {error}")))
}

fn toml_bool(section: &str, key: &str) -> bool {
    match toml_raw_field(section, key) {
        "true" => true,
        "false" => false,
        other => fail(format!("TOML field {key} must be boolean, got {other}")),
    }
}

fn toml_string_array(section: &str, key: &str) -> Vec<String> {
    let value = toml_raw_field(section, key);
    let inner = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .unwrap_or_else(|| fail(format!("TOML field {key} must be an array")));
    if inner.trim().is_empty() {
        return Vec::new();
    }
    inner
        .split(',')
        .map(|item| {
            item.trim()
                .strip_prefix('"')
                .and_then(|rest| rest.strip_suffix('"'))
                .unwrap_or_else(|| fail(format!("TOML field {key} must contain strings")))
                .to_owned()
        })
        .collect()
}

fn json_field_start(raw: &str, key: &str) -> usize {
    let needle = format!("\"{key}\"");
    let pos = raw.find(&needle).unwrap_or_else(|| fail(format!("missing JSON field {key}")));
    raw[pos + needle.len()..]
        .find(':')
        .map(|idx| pos + needle.len() + idx + 1)
        .unwrap_or_else(|| fail(format!("missing ':' after JSON field {key}")))
}

fn json_object<'a>(raw: &'a str, key: &str) -> &'a str {
    let mut idx = json_field_start(raw, key);
    let bytes = raw.as_bytes();
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    require(idx < bytes.len() && bytes[idx] == b'{', format!("JSON field {key} must be object"));
    let start = idx;
    let mut depth = 0i32;
    let mut in_string = false;
    let mut escaped = false;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if in_string {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                in_string = false;
            }
        } else {
            match byte {
                b'"' => in_string = true,
                b'{' => depth += 1,
                b'}' => {
                    depth -= 1;
                    if depth == 0 {
                        return &raw[start..=idx];
                    }
                }
                _ => {}
            }
        }
        idx += 1;
    }
    fail(format!("unterminated JSON object {key}"));
}

fn json_string<'a>(raw: &'a str, key: &str) -> &'a str {
    let mut idx = json_field_start(raw, key);
    let bytes = raw.as_bytes();
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    require(idx < bytes.len() && bytes[idx] == b'"', format!("JSON field {key} must be string"));
    let start = idx + 1;
    idx += 1;
    let mut escaped = false;
    while idx < bytes.len() {
        let byte = bytes[idx];
        if escaped {
            escaped = false;
        } else if byte == b'\\' {
            escaped = true;
        } else if byte == b'"' {
            return &raw[start..idx];
        }
        idx += 1;
    }
    fail(format!("unterminated JSON string {key}"));
}

fn json_number(raw: &str, key: &str) -> f64 {
    let mut idx = json_field_start(raw, key);
    let bytes = raw.as_bytes();
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    let start = idx;
    while idx < bytes.len() && matches!(bytes[idx], b'0'..=b'9' | b'.' | b'-' | b'+' | b'e' | b'E') {
        idx += 1;
    }
    raw[start..idx]
        .parse::<f64>()
        .unwrap_or_else(|error| fail(format!("JSON field {key} must be numeric: {error}")))
}

fn json_bool(raw: &str, key: &str) -> bool {
    let mut idx = json_field_start(raw, key);
    while idx < raw.len() && raw.as_bytes()[idx].is_ascii_whitespace() {
        idx += 1;
    }
    if raw[idx..].starts_with("true") {
        true
    } else if raw[idx..].starts_with("false") {
        false
    } else {
        fail(format!("JSON field {key} must be boolean"));
    }
}

fn json_string_array(raw: &str, key: &str) -> Vec<String> {
    let mut idx = json_field_start(raw, key);
    let bytes = raw.as_bytes();
    while idx < bytes.len() && bytes[idx].is_ascii_whitespace() {
        idx += 1;
    }
    require(idx < bytes.len() && bytes[idx] == b'[', format!("JSON field {key} must be array"));
    let end = raw[idx..]
        .find(']')
        .map(|offset| idx + offset)
        .unwrap_or_else(|| fail(format!("unterminated JSON array {key}")));
    raw[idx + 1..end]
        .split(',')
        .filter_map(|item| {
            let item = item.trim();
            if item.is_empty() {
                None
            } else {
                Some(
                    item.strip_prefix('"')
                        .and_then(|rest| rest.strip_suffix('"'))
                        .unwrap_or_else(|| fail(format!("JSON field {key} must contain strings")))
                        .to_owned(),
                )
            }
        })
        .collect()
}

fn is_lower_sha40(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn main() {
    let root = root();
    require(!root.join(".auth-shared.toml").exists(), "legacy .auth-shared.toml must not exist");

    let mw = read_real_file(&root, ".ores-mw.toml");
    require(toml_int(toml_root(&mw), "schema_version") == 1, "middleware schema drift");
    require(toml_string(toml_root(&mw), "repository_mode") == "server-only", "middleware mode drift");
    require(toml_string(toml_root(&mw), "default_target") == "api", "middleware target drift");
    let target = toml_section(&mw, "[[targets]]");
    require(toml_string(target, "name") == "api", "middleware target name drift");
    require(toml_string(target, "role") == "server", "middleware target role drift");
    require(toml_string(target, "stack_config") == "config/ores-middleware-stack.json", "middleware stack path drift");

    let rl = read_real_file(&root, ".ores-rl.toml");
    require(toml_string(toml_root(&rl), "schemaVersion") == "ores.rate-limit.config.v1", "rate-limit schema drift");
    require(toml_string(toml_root(&rl), "defaultPolicyId") == "public-default", "rate-limit default policy drift");
    let server = toml_section(&rl, "[server]");
    require(toml_string(server, "backend") == "local", "rate-limit backend drift");
    require(toml_string(server, "enforcementLayer") == "service", "rate-limit enforcement layer drift");
    let policy = toml_section(&rl, "[[policies]]");
    require(toml_string(policy, "policyId") == "public-default", "rate-limit policyId drift");
    require(toml_bool(policy, "clientVisible"), "public API rate-limit policy must be client-visible");
    require(toml_string(policy, "algorithm") == "token-bucket", "rate-limit algorithm drift");
    require(toml_int(policy, "capacity") == 120, "public API capacity must be 120");
    require(toml_int(policy, "windowMs") == 0, "token-bucket policy windowMs must remain 0");
    let refill_tokens = toml_int(policy, "refillTokens");
    let refill_interval_ms = toml_int(policy, "refillIntervalMs");
    require(refill_tokens == 120 && refill_interval_ms == 60_000, "public API refill contract drift");
    require(toml_string(policy, "backendFailureMode") == "fail-closed", "rate-limit backend must fail closed");
    require(toml_int(policy, "maxOvershoot") == 0, "rate-limit maxOvershoot must be 0");

    let lru = read_real_file(&root, ".ores-lru.toml");
    let defaults = toml_section(&lru, "[defaults]");
    require(toml_int(defaults, "capacity") == 512, "API LRU capacity must be 512");
    require(toml_string(defaults, "syncMode") == "read_only", "LRU syncMode drift");
    require(!toml_bool(defaults, "failOpenOnStartup"), "LRU startup must fail closed");

    let auth = read_real_file(&root, ".shared-auth.toml");
    require(toml_int(toml_root(&auth), "schema_version") == 1, "shared-auth schema drift");
    let compatibility = toml_section(&auth, "[compatibility]");
    require(
        toml_string(compatibility, "repository") == "https://github.com/shared-auth/shared-auth-interfaces",
        "shared-auth authority drift",
    );
    require(is_lower_sha40(toml_string(compatibility, "commit")), "shared-auth authority must use immutable SHA40");
    require(!toml_bool(toml_section(&auth, "[factors.two_factor]"), "required"), "public API two-factor default drift");
    require(toml_bool(toml_section(&auth, "[factors.three_factor]"), "enabled"), "public API three-factor capability drift");

    let otel = read_real_file(&root, ".ores-otel.toml");
    require(toml_int(toml_root(&otel), "version") == 1, "OTEL config version drift");
    require(toml_bool(toml_section(&otel, "[common]"), "enabled"), "OTEL common must be enabled");
    let tracing = toml_section(&otel, "[common.tracing]");
    require(toml_bool(tracing, "enabled"), "OTEL tracing must be enabled");
    require(
        toml_string_array(tracing, "propagators") == ["tracecontext".to_owned(), "baggage".to_owned()],
        "OTEL propagators drift",
    );
    require(toml_string(toml_section(&otel, "[server]"), "service_name") == "ores-otel-api-server", "OTEL service identity drift");
    require(toml_string(toml_section(&otel, "[server.exporter]"), "protocol") == "none", "API exporter must remain application-owned/disabled by default");

    let stack = read_real_file(&root, "config/ores-middleware-stack.json");
    let rate_limit = json_object(&stack, "rateLimit");
    require(json_bool(rate_limit, "enabled"), "middleware rate limit must be enabled");
    require(json_string(rate_limit, "policyId") == "public-default", "middleware policyId drift");
    require(json_string(rate_limit, "algorithm") == "token-bucket", "middleware algorithm drift");
    require((json_number(rate_limit, "capacity") - 120.0).abs() < f64::EPSILON, "middleware capacity drift");
    let expected_refill = refill_tokens as f64 * 1000.0 / refill_interval_ms as f64;
    require((json_number(rate_limit, "refillPerSecond") - expected_refill).abs() < 1e-12, "middleware refill rate must equal token-bucket authority");
    require(json_string(rate_limit, "failureMode") == "fail-closed", "middleware rate limiter must fail closed");
    require(json_string(rate_limit, "keyNamespace") == "ores-otel-api-server:public-default", "middleware rate-limit namespace drift");
    require(json_string(rate_limit, "keyVersion") == "v1", "middleware keyVersion drift");

    let idempotency = json_object(&stack, "idempotency");
    require(json_bool(idempotency, "enabled"), "write API idempotency must be enabled");
    require(
        json_string_array(idempotency, "requiredMethods") == ["POST".to_owned(), "PUT".to_owned(), "PATCH".to_owned()],
        "API idempotency method contract drift",
    );

    let bypass = json_object(&stack, "testAuthBypass");
    require(!json_bool(bypass, "enabled"), "test auth bypass must remain disabled");
    let bypass_header = json_string(bypass, "headerName");
    require(bypass_header == "x-ores-test-auth-bypass", "test auth bypass header must use reserved x-ores-* namespace");
    require(bypass_header == bypass_header.to_ascii_lowercase(), "extension header must be canonical lowercase");

    let integrations = json_object(&stack, "integrations");
    for name in ["sharedAuth", "optoSync"] {
        let integration = json_object(integrations, name);
        require(json_string(integration, "mode") == "disabled", format!("{name} mode drift"));
        require(!json_bool(integration, "failOpen"), format!("disabled {name} integration must fail closed"));
    }
    let stack_otel = json_object(integrations, "oresOtel");
    require(json_bool(stack_otel, "enabled"), "middleware OTEL integration must be enabled");
    require(json_string(stack_otel, "serviceName") == "ores-otel-api-server", "middleware OTEL service identity drift");

    let security_headers = json_object(&stack, "securityHeaders");
    let csp = json_string(security_headers, "contentSecurityPolicy");
    require(csp.contains("default-src 'none'"), "API CSP must default-deny");
    require(csp.contains("frame-ancestors 'none'"), "API CSP must deny framing");
    require(!csp.contains("script-src") && !csp.contains("style-src"), "API CSP must not authorize UI script/style sources");

    println!("runtime-config conformance: ok");
}
