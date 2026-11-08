//! `ks show <name> [-f field] [-c]` — display or copy an entry.

use ks::{ErrorCode, Kind};

use super::*;

/// Prints the entry: full text, one field, raw binary bytes, or `-c` copies.
pub fn run(cli: &Cli, out: &mut Out, n: &str, field: Option<&str>, copy: bool) -> CliResult<i32> {
    let session = unlock(cli, out)?;
    let name = name(n)?;
    let entry = session.get(&name).map_err(|e| err(&e))?;

    if copy {
        let value = copy_value(&entry, field)?;
        clipboard::copy(value.as_bytes())?;
        return Ok(out.ok_value(
            "copied",
            serde_json::json!({ "name": name.to_string(), "copied": true }),
        ));
    }

    if entry.kind() == Kind::Binary {
        if field.is_some() {
            return fail(ErrorCode::Usage, "-f is not supported for binary entries");
        }
        use base64::Engine as _;
        let b64 = base64::engine::general_purpose::STANDARD.encode(entry.payload());
        if out.json {
            return Ok(out.ok_value(
                "",
                serde_json::json!({ "name": name.to_string(), "type": "binary", "base64": b64 }),
            ));
        }
        use std::io::Write as _;
        std::io::stdout()
            .write_all(entry.payload())
            .and_then(|()| std::io::stdout().flush())
            .map_err(|e| {
                err(&ks::Error::Io {
                    path: "stdout".into(),
                    source: e,
                })
            })?;
        return Ok(0);
    }

    let text = entry
        .as_text()
        .ok_or((ErrorCode::Internal, "text entry without text".to_owned()))?;
    if let Some(field) = field {
        let value = text
            .field(field)
            .ok_or_else(|| err(&ks::Error::NotFound(format!("field {field:?} in {name}"))))?;
        return Ok(out.ok_value(
            value,
            serde_json::json!({ "name": name.to_string(), "field": field, "value": value }),
        ));
    }
    let fields: serde_json::Map<String, serde_json::Value> = text
        .fields()
        .map(|(k, v)| (k.to_owned(), serde_json::Value::from(v)))
        .collect();
    Ok(out.ok_value(
        text.as_str(),
        serde_json::json!({
            "name": name.to_string(),
            "type": entry.kind().as_str(),
            "value": text.value(),
            "fields": fields,
            "text": text.as_str(),
        }),
    ))
}

fn copy_value(entry: &ks::Entry, field: Option<&str>) -> CliResult<String> {
    match (entry.kind(), entry.as_text()) {
        (Kind::Binary, _) => fail(ErrorCode::Usage, "-c is not supported for binary entries"),
        (_, Some(text)) => match field {
            Some(f) => text
                .field(f)
                .map(str::to_owned)
                .ok_or_else(|| err(&ks::Error::NotFound(format!("field {f:?}")))),
            None => Ok(text.value().to_owned()),
        },
        (_, None) => fail(ErrorCode::Internal, "text entry without text"),
    }
}
