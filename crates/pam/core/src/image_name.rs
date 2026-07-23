pub fn parse_image_file_name(value: &str) -> String {
    let mut result = value.to_string();
    if let (Some(open), Some(close)) = (result.find('('), result.find(')'))
        && close >= open
    {
        result.replace_range(open..=close, "");
    }
    if let Some(dollar) = result.find('$') {
        result = result[dollar + 1..].to_string();
    }
    if let (Some(open), Some(close)) = (result.find('['), result.find(']'))
        && close >= open
    {
        result.replace_range(open..=close, "");
    }
    if let Some(pipe) = result.find('|') {
        result.truncate(pipe);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_popcap_image_names() {
        assert_eq!(parse_image_file_name("IMAGE_REANIM(12)"), "IMAGE_REANIM");
        assert_eq!(parse_image_file_name("atlas$IMAGE[3]|fallback"), "IMAGE");
    }
}
