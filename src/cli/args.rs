/// Parse percentage value (0-100) to decimal (0.0-1.0)
pub fn parse_percentage(s: &str) -> Result<f64, String> {
    let value: f64 = s
        .parse()
        .map_err(|_| format!("'{}' is not a valid number", s))?;

    if value < 0.0 || value > 100.0 {
        return Err(format!("Price must be between 0 and 100, got {}", value));
    }

    Ok(value / 100.0)
}
