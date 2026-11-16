use super::*;

pub(crate) struct Reply {
    pub id: String,
    pub plain: Zeroizing<String>,
    pub cipher: String,
}

/// Parse SSE independently of network chunk boundaries. Buffering is bounded by the HTTP
/// host AND here. Never interpret unencrypted content as a successful encrypted reply.
pub(crate) fn parse(bytes: &[u8], recipient: &crypto::Recipient, model: &str) -> Result<Reply> {
    if bytes.len() > MAX_HTTP_BYTES {
        return Err(Error::Limit);
    }
    let text = std::str::from_utf8(bytes).map_err(|_| Error::Protocol)?;
    let mut reply = Reply {
        id: String::new(),
        plain: Zeroizing::new(String::new()),
        cipher: String::new(),
    };
    let mut event = String::new();
    let mut done = false;
    let mut finished = false;
    let mut chunks = 0;
    for line in text.lines().chain(std::iter::once("")) {
        if line.len() > 1024 * 1024 {
            return Err(Error::Limit);
        }
        if let Some(data) = line.strip_prefix("data:") {
            if !event.is_empty() {
                event.push('\n');
            }
            event.push_str(data.strip_prefix(' ').unwrap_or(data));
            if event.len() > 1024 * 1024 {
                return Err(Error::Limit);
            }
        } else if line.is_empty() && !event.is_empty() {
            if done {
                return Err(Error::Protocol);
            }
            if event == "[DONE]" {
                done = true;
                event.clear();
                continue;
            }
            let v: Value = serde_json::from_str(&event).map_err(|_| Error::Protocol)?;
            event.clear();
            if !v.get("error").is_none_or(Value::is_null) {
                return Err(Error::Protocol);
            }
            if v.get("model")
                .and_then(Value::as_str)
                .is_some_and(|s| s != model)
            {
                return Err(Error::Protocol);
            }
            let id = v.get("id").and_then(Value::as_str).ok_or(Error::Protocol)?;
            if id.is_empty() || id.len() > 200 || id.chars().any(char::is_control) {
                return Err(Error::Protocol);
            }
            if reply.id.is_empty() {
                reply.id = id.into();
            } else if reply.id != id {
                return Err(Error::Protocol);
            }
            let choices = v
                .get("choices")
                .and_then(Value::as_array)
                .ok_or(Error::Protocol)?;
            if choices.is_empty() {
                continue;
            } // usage-only final event
            if choices.len() != 1 || choices[0].get("index").and_then(Value::as_u64) != Some(0) {
                return Err(Error::Protocol);
            }
            let choice = &choices[0];
            if let Some(reason) = choice.get("finish_reason").filter(|v| !v.is_null()) {
                if reason.as_str() != Some("stop") || finished {
                    return Err(Error::Protocol);
                }
                finished = true;
            }
            if let Some(delta) = choice.get("delta") {
                if delta.get("tool_calls").is_some() || delta.get("function_call").is_some() {
                    return Err(Error::Protocol);
                }
                // Never pass a reasoning side-channel through as plaintext either.
                for field in ["content", "reasoning_content", "reasoning"] {
                    if let Some(value) = delta.get(field).filter(|v| !v.is_null()) {
                        let cipher = value.as_str().ok_or(Error::Protocol)?;
                        if cipher.is_empty() {
                            continue;
                        }
                        chunks += 1;
                        if chunks > 8192 {
                            return Err(Error::Limit);
                        }
                        let plain = Zeroizing::new(recipient.decrypt(cipher)?);
                        reply.cipher.push_str(cipher);
                        if field == "content" {
                            reply.plain.push_str(&plain);
                        }
                        if reply.plain.len() > MAX_REPLY_BYTES {
                            return Err(Error::Limit);
                        }
                    }
                }
            }
        } else if !line.is_empty()
            && !line.starts_with(':')
            && !line.starts_with("event:")
            && !line.starts_with("id:")
            && !line.starts_with("retry:")
        {
            return Err(Error::Protocol);
        }
    }
    if !done || !finished || reply.id.is_empty() || reply.plain.trim().is_empty() {
        return Err(Error::Protocol);
    }
    Ok(reply)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn stream(cipher: &str) -> String {
        format!(
            "data: {}\r\n\r\ndata: {}\r\n\r\ndata: [DONE]\r\n\r\n",
            json!({"id":"fresh-request","model":DEFAULT_MODEL,"choices":[{"index":0,"delta":{"content":cipher},"finish_reason":null}]}),
            json!({"id":"fresh-request","model":DEFAULT_MODEL,"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]})
        )
    }
    #[test]
    fn decrypts_only_complete_authenticated_frames() {
        let recipient = crypto::Recipient::new();
        let public = crypto::public_key(&recipient.public_hex()).unwrap();
        let cipher = crypto::encrypt("A private reply", &public).unwrap();
        let wire = stream(&cipher);
        assert_eq!(
            &*parse(wire.as_bytes(), &recipient, DEFAULT_MODEL)
                .unwrap()
                .plain,
            "A private reply"
        );
        assert!(parse(
            wire.replace("data: [DONE]", "").as_bytes(),
            &recipient,
            DEFAULT_MODEL
        )
        .is_err());
        assert!(parse(
            wire.replace("\"stop\"", "\"length\"").as_bytes(),
            &recipient,
            DEFAULT_MODEL
        )
        .is_err());
        assert!(parse(
            wire.replace(DEFAULT_MODEL, "e2ee-wrong").as_bytes(),
            &recipient,
            DEFAULT_MODEL
        )
        .is_err());
        assert!(parse(
            stream("unencrypted fallback").as_bytes(),
            &recipient,
            DEFAULT_MODEL
        )
        .is_err());
        assert!(parse(wire.as_bytes(), &crypto::Recipient::new(), DEFAULT_MODEL).is_err());
        assert!(parse(
            format!("{wire}data: {{}}\n\n").as_bytes(),
            &recipient,
            DEFAULT_MODEL
        )
        .is_err());
    }
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn rejects_plaintext_truncation_and_mixed_request_ids() {
        let key = crypto::Recipient::new();
        let public = crypto::public_key(&key.public_hex()).unwrap();
        let encrypted = crypto::encrypt("Private answer", &public).unwrap();
        let event = |content: &str| {
            format!(
                "data: {}\r\n\r\n",
                json!({"id":"chat-1","model":DEFAULT_MODEL,"choices":[{"index":0,"delta":{"content":content},"finish_reason":"stop"}]})
            )
        };
        let good = event(&encrypted) + "data: [DONE]\r\n\r\n";
        assert_eq!(
            parse(good.as_bytes(), &key, DEFAULT_MODEL)
                .unwrap()
                .plain
                .as_str(),
            "Private answer"
        );
        assert!(parse(event("plaintext").as_bytes(), &key, DEFAULT_MODEL).is_err());
        assert!(parse(event(&encrypted).as_bytes(), &key, DEFAULT_MODEL).is_err());
        let mixed =
            event(&encrypted) + &event(&encrypted).replace("chat-1", "chat-2") + "data: [DONE]\n\n";
        assert!(parse(mixed.as_bytes(), &key, DEFAULT_MODEL).is_err());
    }
}
