/// Converts LaTeX math strings to Unicode text for terminal display.
///
/// Used when the user copies equations from the board to paste into terminal chat.
pub fn latex_to_unicode(latex: &str) -> String {
    let mut s = latex.to_string();

    // Strip delimiter sizing commands
    for cmd in &[
        "\\Bigg", "\\bigg", "\\Big", "\\big", "\\left", "\\right",
    ] {
        s = s.replace(cmd, "");
    }

    // Strip spacing commands
    for cmd in &[
        "\\qquad",
        "\\quad",
        "\\displaystyle",
        "\\textstyle",
        "\\,",
        "\\;",
        "\\:",
        "\\!",
    ] {
        s = s.replace(cmd, "");
    }

    // Handle matrices before general processing
    s = convert_matrices(&s);

    // Process structural commands via char-by-char walk
    s = process_structural(&s);

    // Replace Greek letters and symbols (longest match first)
    s = replace_symbols(&s);

    // Cleanup: collapse whitespace, trim
    s = cleanup(&s);

    s
}

// ---------------------------------------------------------------------------
// Symbol replacement
// ---------------------------------------------------------------------------

/// Pre-sorted symbol table (sorted once, reused on every call).
fn symbol_table() -> &'static [(&'static str, &'static str)] {
    use std::sync::OnceLock;
    static TABLE: OnceLock<Vec<(&str, &str)>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut table: Vec<(&str, &str)> = GREEK_LOWER
            .iter()
            .chain(GREEK_UPPER.iter())
            .chain(SYMBOLS.iter())
            .chain(OPERATORS.iter())
            .chain(ARROWS.iter())
            .chain(DELIMITERS.iter())
            .copied()
            .collect();
        table.sort_by(|a, b| b.0.len().cmp(&a.0.len()));
        table
    })
}

fn replace_symbols(input: &str) -> String {
    let table = symbol_table();

    let mut result = String::with_capacity(input.len());
    let mut i = 0;
    while i < input.len() {
        if input.as_bytes()[i] == b'\\' {
            let mut matched = false;
            for &(cmd, repl) in table {
                if input[i..].starts_with(cmd) {
                    // Make sure the match isn't a prefix of a longer command name
                    let end = i + cmd.len();
                    if cmd.as_bytes().last().is_some_and(|b: &u8| b.is_ascii_alphabetic())
                        && end < input.len()
                        && input.as_bytes()[end].is_ascii_alphabetic()
                    {
                        continue;
                    }
                    result.push_str(repl);
                    i = end;
                    // Consume the optional space that LaTeX uses as command terminator,
                    // but ONLY if the next non-space char is a letter/digit (another
                    // variable or command). If it's an operator like =, +, -, keep
                    // the space so "ψ = E" doesn't become "ψ= E".
                    if i < input.len() && input.as_bytes()[i] == b' ' {
                        let next_non_space = input[i..].bytes()
                            .skip(1)
                            .find(|b| !b.is_ascii_whitespace());
                        let next_is_letter_or_cmd = next_non_space
                            .map(|b| b.is_ascii_alphabetic() || b == b'\\')
                            .unwrap_or(false);
                        if next_is_letter_or_cmd {
                            i += 1;
                        }
                    }
                    matched = true;
                    break;
                }
            }
            if !matched {
                result.push('\\');
                i += 1;
            }
        } else {
            let ch = input[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }
    result
}

const GREEK_LOWER: &[(&str, &str)] = &[
    ("\\varepsilon", "\u{03B5}"),
    ("\\vartheta", "\u{03D1}"),
    ("\\varphi", "\u{03C6}"),
    ("\\alpha", "\u{03B1}"),
    ("\\beta", "\u{03B2}"),
    ("\\gamma", "\u{03B3}"),
    ("\\delta", "\u{03B4}"),
    ("\\epsilon", "\u{03B5}"),
    ("\\zeta", "\u{03B6}"),
    ("\\eta", "\u{03B7}"),
    ("\\theta", "\u{03B8}"),
    ("\\iota", "\u{03B9}"),
    ("\\kappa", "\u{03BA}"),
    ("\\lambda", "\u{03BB}"),
    ("\\mu", "\u{03BC}"),
    ("\\nu", "\u{03BD}"),
    ("\\xi", "\u{03BE}"),
    ("\\pi", "\u{03C0}"),
    ("\\rho", "\u{03C1}"),
    ("\\sigma", "\u{03C3}"),
    ("\\tau", "\u{03C4}"),
    ("\\upsilon", "\u{03C5}"),
    ("\\phi", "\u{03C6}"),
    ("\\chi", "\u{03C7}"),
    ("\\psi", "\u{03C8}"),
    ("\\omega", "\u{03C9}"),
];

const GREEK_UPPER: &[(&str, &str)] = &[
    ("\\Gamma", "\u{0393}"),
    ("\\Delta", "\u{0394}"),
    ("\\Theta", "\u{0398}"),
    ("\\Lambda", "\u{039B}"),
    ("\\Xi", "\u{039E}"),
    ("\\Pi", "\u{03A0}"),
    ("\\Sigma", "\u{03A3}"),
    ("\\Upsilon", "\u{03A5}"),
    ("\\Phi", "\u{03A6}"),
    ("\\Psi", "\u{03A8}"),
    ("\\Omega", "\u{03A9}"),
];

const SYMBOLS: &[(&str, &str)] = &[
    ("\\emptyset", "\u{2205}"),
    ("\\parallel", "\u{2225}"),
    ("\\partial", "\u{2202}"),
    ("\\ddagger", "\u{2021}"),
    ("\\forall", "\u{2200}"),
    ("\\exists", "\u{2203}"),
    ("\\approx", "\u{2248}"),
    ("\\bullet", "\u{2022}"),
    ("\\dagger", "\u{2020}"),
    ("\\otimes", "\u{2297}"),
    ("\\oplus", "\u{2295}"),
    ("\\supset", "\u{2283}"),
    ("\\subset", "\u{2282}"),
    ("\\propto", "\u{221D}"),
    ("\\infty", "\u{221E}"),
    ("\\nabla", "\u{2207}"),
    ("\\notin", "\u{2209}"),
    ("\\equiv", "\u{2261}"),
    ("\\wedge", "\u{2227}"),
    ("\\prime", "\u{2032}"),
    ("\\times", "\u{00D7}"),
    ("\\hbar", "\u{210F}"),
    ("\\cdot", "\u{00B7}"),
    ("\\circ", "\u{2218}"),
    ("\\star", "\u{22C6}"),
    ("\\perp", "\u{22A5}"),
    ("\\ell", "\u{2113}"),
    ("\\neg", "\u{00AC}"),
    ("\\vee", "\u{2228}"),
    ("\\cup", "\u{222A}"),
    ("\\cap", "\u{2229}"),
    ("\\neq", "\u{2260}"),
    ("\\leq", "\u{2264}"),
    ("\\geq", "\u{2265}"),
    ("\\sim", "\u{223C}"),
    ("\\in", "\u{2208}"),
    ("\\ll", "\u{226A}"),
    ("\\gg", "\u{226B}"),
    ("\\pm", "\u{00B1}"),
    ("\\mp", "\u{2213}"),
];

const OPERATORS: &[(&str, &str)] = &[
    ("\\oint", "\u{222E}"),
    ("\\prod", "\u{220F}"),
    ("\\int", "\u{222B}"),
    ("\\sum", "\u{2211}"),
    ("\\lim", "lim"),
];

const ARROWS: &[(&str, &str)] = &[
    ("\\Leftrightarrow", "\u{21D4}"),
    ("\\leftrightarrow", "\u{2194}"),
    ("\\Rightarrow", "\u{21D2}"),
    ("\\rightarrow", "\u{2192}"),
    ("\\Leftarrow", "\u{21D0}"),
    ("\\leftarrow", "\u{2190}"),
    ("\\downarrow", "\u{2193}"),
    ("\\uparrow", "\u{2191}"),
    ("\\mapsto", "\u{21A6}"),
    ("\\to", "\u{2192}"),
];

const DELIMITERS: &[(&str, &str)] = &[
    ("\\langle", "\u{27E8}"),
    ("\\rangle", "\u{27E9}"),
    ("\\{", "{"),
    ("\\}", "}"),
];

// ---------------------------------------------------------------------------
// Brace-group helper
// ---------------------------------------------------------------------------

/// Extract content between matched `{...}` starting at `start`.
/// Returns `(inner_content, index_after_closing_brace)`.
fn find_brace_group(s: &str, start: usize) -> Option<(String, usize)> {
    let bytes = s.as_bytes();
    if start >= bytes.len() || bytes[start] != b'{' {
        return None;
    }
    let mut depth = 0;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'{' => depth += 1,
            b'}' => {
                depth -= 1;
                if depth == 0 {
                    let inner = &s[start + 1..i];
                    return Some((inner.to_string(), i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Structural processing (frac, sqrt, super/sub-scripts, accents, text cmds)
// ---------------------------------------------------------------------------

fn process_structural(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut i = 0;

    while i < input.len() {
        let b = input.as_bytes()[i];
        if b == b'\\' {
            // \frac{num}{den}
            if input[i..].starts_with("\\frac") && !input[i..].starts_with("\\fractal") {
                let after = i + 5;
                if let Some((num, end1)) = find_brace_group(input, after) {
                    if let Some((den, end2)) = find_brace_group(input, end1) {
                        let num_u = process_structural(&num);
                        let den_u = process_structural(&den);
                        let needs_num_parens = is_complex(&num_u);
                        let needs_den_parens = is_complex(&den_u);
                        if needs_num_parens {
                            result.push('(');
                        }
                        result.push_str(&num_u);
                        if needs_num_parens {
                            result.push(')');
                        }
                        result.push('/');
                        if needs_den_parens {
                            result.push('(');
                        }
                        result.push_str(&den_u);
                        if needs_den_parens {
                            result.push(')');
                        }
                        i = end2;
                        continue;
                    }
                }
            }

            // \sqrt{...}
            if input[i..].starts_with("\\sqrt") {
                let after = i + 5;
                if let Some((content, end)) = find_brace_group(input, after) {
                    let inner = process_structural(&content);
                    result.push('\u{221A}');
                    if is_complex(&inner) {
                        result.push('(');
                        result.push_str(&inner);
                        result.push(')');
                    } else {
                        result.push_str(&inner);
                    }
                    i = end;
                    continue;
                }
            }

            // Accent commands: \hat, \vec, \bar, \dot, \tilde
            if let Some((cmd, combining)) = match_accent(&input[i..]) {
                let after = i + cmd.len();
                if let Some((content, end)) = find_brace_group(input, after) {
                    let inner = process_structural(&content);
                    result.push_str(&inner);
                    result.push(combining);
                    i = end;
                    continue;
                }
            }

            // Text/font commands: \text, \mathbf, \mathrm, \mathcal, \boldsymbol, \operatorname
            if let Some(cmd_len) = match_text_command(&input[i..]) {
                let after = i + cmd_len;
                if let Some((content, end)) = find_brace_group(input, after) {
                    let inner = process_structural(&content);
                    result.push_str(&inner);
                    i = end;
                    continue;
                }
            }

            // \\ (line break in non-matrix context) -> space
            if input[i..].starts_with("\\\\") {
                result.push(' ');
                i += 2;
                continue;
            }

            // Pass through the backslash; symbol replacement happens later
            result.push('\\');
            i += 1;
        } else if b == b'^' {
            i += 1;
            let (content, new_i) = read_script_arg(input, i);
            let inner = process_structural(&content);
            let converted = to_superscript(&inner);
            result.push_str(&converted);
            i = new_i;
        } else if b == b'_' {
            i += 1;
            let (content, new_i) = read_script_arg(input, i);
            let inner = process_structural(&content);
            let converted = to_subscript(&inner);
            result.push_str(&converted);
            i = new_i;
        } else if b == b'&' {
            result.push(' ');
            i += 1;
        } else {
            let ch = input[i..].chars().next().unwrap();
            result.push(ch);
            i += ch.len_utf8();
        }
    }

    result
}

/// Read the argument to ^ or _: either a brace group or a single char.
fn read_script_arg(s: &str, pos: usize) -> (String, usize) {
    if pos >= s.len() {
        return (String::new(), pos);
    }
    if s.as_bytes()[pos] == b'{' {
        if let Some((content, end)) = find_brace_group(s, pos) {
            return (content, end);
        }
    }
    // Single character (or backslash command for a single token)
    if s.as_bytes()[pos] == b'\\' {
        let mut end = pos + 1;
        while end < s.len() && s.as_bytes()[end].is_ascii_alphabetic() {
            end += 1;
        }
        if end == pos + 1 {
            // Backslash followed by non-alpha, take two chars
            if end < s.len() {
                end += 1;
            }
        }
        return (s[pos..end].to_string(), end);
    }
    // Single non-backslash character
    let ch = s[pos..].chars().next().unwrap();
    (ch.to_string(), pos + ch.len_utf8())
}

fn match_accent(s: &str) -> Option<(&'static str, char)> {
    const ACCENTS: &[(&str, char)] = &[
        ("\\tilde", '\u{0303}'),
        ("\\hat", '\u{0302}'),
        ("\\vec", '\u{20D7}'),
        ("\\bar", '\u{0304}'),
        ("\\dot", '\u{0307}'),
    ];
    for &(cmd, combining) in ACCENTS {
        if let Some(rest) = s.strip_prefix(cmd) {
            if rest.starts_with('{')
                || rest.is_empty()
                || !rest.as_bytes()[0].is_ascii_alphabetic()
            {
                return Some((cmd, combining));
            }
        }
    }
    None
}

fn match_text_command(s: &str) -> Option<usize> {
    const COMMANDS: &[&str] = &[
        "\\operatorname",
        "\\boldsymbol",
        "\\mathrm",
        "\\mathbf",
        "\\mathcal",
        "\\mathit",
        "\\text",
    ];
    for cmd in COMMANDS {
        if let Some(rest) = s.strip_prefix(cmd) {
            if rest.starts_with('{') {
                return Some(cmd.len());
            }
        }
    }
    None
}

/// Determine if an expression is "complex" (needs parentheses in frac/sqrt).
fn is_complex(s: &str) -> bool {
    let trimmed = s.trim();
    if trimmed.chars().count() <= 1 {
        return false;
    }
    trimmed.contains('+')
        || trimmed.contains('-')
        || trimmed.contains('/')
        || trimmed.contains('*')
        || trimmed.contains('\u{00B1}')
        || trimmed.contains('\u{2213}')
}

// ---------------------------------------------------------------------------
// Superscript / subscript conversion
// ---------------------------------------------------------------------------

fn to_superscript(s: &str) -> String {
    let mut result = String::new();
    let mut all_converted = true;

    for ch in s.chars() {
        match superscript_char(ch) {
            Some(sup) => result.push(sup),
            None => {
                all_converted = false;
                break;
            }
        }
    }

    if all_converted {
        result
    } else {
        format!("^({})", s)
    }
}

fn to_subscript(s: &str) -> String {
    let mut result = String::new();
    let mut all_converted = true;

    for ch in s.chars() {
        match subscript_char(ch) {
            Some(sub) => result.push(sub),
            None => {
                all_converted = false;
                break;
            }
        }
    }

    if all_converted {
        result
    } else {
        format!("_({})", s)
    }
}

fn superscript_char(ch: char) -> Option<char> {
    match ch {
        '0' => Some('\u{2070}'),
        '1' => Some('\u{00B9}'),
        '2' => Some('\u{00B2}'),
        '3' => Some('\u{00B3}'),
        '4' => Some('\u{2074}'),
        '5' => Some('\u{2075}'),
        '6' => Some('\u{2076}'),
        '7' => Some('\u{2077}'),
        '8' => Some('\u{2078}'),
        '9' => Some('\u{2079}'),
        'n' => Some('\u{207F}'),
        'i' => Some('\u{2071}'),
        '+' => Some('\u{207A}'),
        '-' => Some('\u{207B}'),
        '=' => Some('\u{207C}'),
        '(' => Some('\u{207D}'),
        ')' => Some('\u{207E}'),
        _ => None,
    }
}

fn subscript_char(ch: char) -> Option<char> {
    match ch {
        '0' => Some('\u{2080}'),
        '1' => Some('\u{2081}'),
        '2' => Some('\u{2082}'),
        '3' => Some('\u{2083}'),
        '4' => Some('\u{2084}'),
        '5' => Some('\u{2085}'),
        '6' => Some('\u{2086}'),
        '7' => Some('\u{2087}'),
        '8' => Some('\u{2088}'),
        '9' => Some('\u{2089}'),
        'a' => Some('\u{2090}'),
        'e' => Some('\u{2091}'),
        'i' => Some('\u{1D62}'),
        'n' => Some('\u{2099}'),
        'o' => Some('\u{2092}'),
        'r' => Some('\u{1D63}'),
        's' => Some('\u{209B}'),
        'x' => Some('\u{2093}'),
        '+' => Some('\u{208A}'),
        '-' => Some('\u{208B}'),
        '=' => Some('\u{208C}'),
        '(' => Some('\u{208D}'),
        ')' => Some('\u{208E}'),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Matrix conversion
// ---------------------------------------------------------------------------

fn convert_matrices(input: &str) -> String {
    let mut s = input.to_string();

    for (env, open, close) in &[
        ("pmatrix", "(", ")"),
        ("bmatrix", "[", "]"),
        ("vmatrix", "|", "|"),
        ("Bmatrix", "{", "}"),
        ("matrix", "", ""),
    ] {
        let begin = format!("\\begin{{{}}}", env);
        let end_tag = format!("\\end{{{}}}", env);

        while let Some(start) = s.find(&begin) {
            let Some(rel_end) = s[start..].find(&end_tag) else {
                break;
            };
            let end = start + rel_end;

            let inner = &s[start + begin.len()..end];
            let converted = convert_matrix_inner(inner, open, close);
            s = format!("{}{}{}", &s[..start], converted, &s[end + end_tag.len()..]);
        }
    }

    s
}

fn convert_matrix_inner(inner: &str, open: &str, close: &str) -> String {
    let rows: Vec<&str> = inner.split("\\\\").collect();
    let mut row_strs = Vec::new();
    for row in &rows {
        let cols: Vec<&str> = row.split('&').map(|c| c.trim()).collect();
        let non_empty: Vec<&str> = cols.into_iter().filter(|c| !c.is_empty()).collect();
        if !non_empty.is_empty() {
            row_strs.push(non_empty.join(" "));
        }
    }
    format!("{}{}{}", open, row_strs.join("; "), close)
}

// ---------------------------------------------------------------------------
// Final cleanup
// ---------------------------------------------------------------------------

fn cleanup(s: &str) -> String {
    // In math mode, most spaces are formatting artifacts. We keep a space only
    // when both the preceding and following characters are "space-worthy" — i.e.
    // ASCII alphanumeric or common punctuation that benefits from visual separation.
    let chars: Vec<char> = s.chars().collect();
    let mut result = String::with_capacity(s.len());
    let mut i = 0;

    while i < chars.len() {
        if chars[i].is_whitespace() {
            // Collapse whitespace run
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }
            let before = result.chars().last();
            let after = chars.get(i).copied();
            let keep = match (before, after) {
                (Some(b), Some(a)) => is_space_worthy(b) && is_space_worthy(a),
                _ => false,
            };
            if keep {
                result.push(' ');
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result.trim().to_string()
}

/// Characters that want spaces preserved between them.
fn is_space_worthy(ch: char) -> bool {
    ch.is_alphanumeric()  // includes Unicode letters like ψ, α, Ĥ
        || ch == '('
        || ch == ')'
        || ch == '['
        || ch == ']'
        || ch == '='
        || ch == ':'
        || ch == ';'
        || ch == ','
        || ch == '.'
        || ch == '+'
        || ch == '-'
        || ch == '/'
        || ch == '\u{0302}' // combining circumflex (hat)
        || ch == '\u{0304}' // combining macron (bar)
        || ch == '\u{0303}' // combining tilde
        || ch == '\u{0307}' // combining dot
        || ch == '\u{20D7}' // combining arrow (vec)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_greek_letters() {
        assert_eq!(latex_to_unicode("\\alpha + \\beta"), "α + β");
        assert_eq!(latex_to_unicode("\\Omega"), "\u{03A9}");
        assert_eq!(latex_to_unicode("\\varepsilon"), "\u{03B5}");
    }

    #[test]
    fn test_symbols() {
        assert_eq!(latex_to_unicode("\\infty"), "\u{221E}");
        assert_eq!(
            latex_to_unicode("\\nabla \\cdot E"),
            "\u{2207}\u{00B7}E"
        );
        assert_eq!(latex_to_unicode("a \\neq b"), "a\u{2260}b");
        assert_eq!(latex_to_unicode("x \\in S"), "x\u{2208}S");
    }

    #[test]
    fn test_superscripts() {
        assert_eq!(latex_to_unicode("x^2"), "x\u{00B2}");
        assert_eq!(
            latex_to_unicode("e^{i\\pi}"),
            "e^(i\u{03C0})"
        );
        assert_eq!(latex_to_unicode("x^{10}"), "x\u{00B9}\u{2070}");
    }

    #[test]
    fn test_subscripts() {
        assert_eq!(latex_to_unicode("x_0"), "x\u{2080}");
        assert_eq!(latex_to_unicode("a_{12}"), "a\u{2081}\u{2082}");
        assert_eq!(latex_to_unicode("x_n"), "x\u{2099}");
    }

    #[test]
    fn test_fractions() {
        assert_eq!(latex_to_unicode("\\frac{a}{b}"), "a/b");
        assert_eq!(latex_to_unicode("\\frac{a+b}{c+d}"), "(a+b)/(c+d)");
        assert_eq!(
            latex_to_unicode("\\frac{\\hbar^2}{2m}"),
            "\u{210F}\u{00B2}/2m"
        );
    }

    #[test]
    fn test_sqrt() {
        assert_eq!(latex_to_unicode("\\sqrt{x}"), "\u{221A}x");
        assert_eq!(latex_to_unicode("\\sqrt{x+y}"), "\u{221A}(x+y)");
    }

    #[test]
    fn test_accents() {
        let result = latex_to_unicode("\\hat{x}");
        assert!(result.contains('x'));
        assert!(result.contains('\u{0302}'));

        let result = latex_to_unicode("\\vec{v}");
        assert!(result.contains('v'));
        assert!(result.contains('\u{20D7}'));
    }

    #[test]
    fn test_text_commands() {
        assert_eq!(latex_to_unicode("\\text{hello}"), "hello");
        assert_eq!(latex_to_unicode("\\mathbf{F}"), "F");
    }

    #[test]
    fn test_delimiters() {
        assert_eq!(latex_to_unicode("\\left( x \\right)"), "( x )");
        assert_eq!(
            latex_to_unicode("\\langle x \\rangle"),
            "\u{27E8}x\u{27E9}"
        );
    }

    #[test]
    fn test_matrices() {
        let input = "\\begin{pmatrix} a & b \\\\ c & d \\end{pmatrix}";
        assert_eq!(latex_to_unicode(input), "(a b; c d)");
    }

    #[test]
    fn test_arrows() {
        assert_eq!(latex_to_unicode("A \\to B"), "A\u{2192}B");
        assert_eq!(latex_to_unicode("A \\Rightarrow B"), "A\u{21D2}B");
        assert_eq!(latex_to_unicode("f: A \\mapsto B"), "f: A\u{21A6}B");
    }

    #[test]
    fn test_complex_expression() {
        // E = mc^2
        assert_eq!(latex_to_unicode("E = mc^2"), "E = mc\u{00B2}");
        // Schrodinger
        let input = "i\\hbar \\frac{\\partial}{\\partial t} \\Psi = \\hat{H} \\Psi";
        let result = latex_to_unicode(input);
        assert!(result.contains("i\u{210F}"));
        assert!(result.contains("\u{2202}/\u{2202}"));
        assert!(result.contains("\u{03A8}"));
    }

    #[test]
    fn test_spacing_stripped() {
        assert_eq!(latex_to_unicode("a \\quad b"), "a b");
        assert_eq!(latex_to_unicode("a \\, b"), "a b");
    }

    #[test]
    fn test_integral() {
        assert_eq!(
            latex_to_unicode("\\int_0^\\infty"),
            "\u{222B}\u{2080}^(\u{221E})"
        );
    }

    #[test]
    fn test_brace_group_helper() {
        assert_eq!(
            find_brace_group("{hello}", 0),
            Some(("hello".to_string(), 7))
        );
        assert_eq!(
            find_brace_group("{a{b}c}", 0),
            Some(("a{b}c".to_string(), 7))
        );
        assert_eq!(find_brace_group("no braces", 0), None);
    }

    #[test]
    fn test_greek_upper() {
        assert_eq!(latex_to_unicode("\\Gamma"), "\u{0393}");
        assert_eq!(latex_to_unicode("\\Delta"), "\u{0394}");
        assert_eq!(latex_to_unicode("\\Psi"), "\u{03A8}");
    }

    #[test]
    fn test_nested_fractions() {
        // \frac{\hbar^2}{2m} → ℏ²/2m
        let result = latex_to_unicode("\\frac{\\hbar^2}{2m}");
        assert_eq!(result, "\u{210F}\u{00B2}/2m");
    }

    #[test]
    fn test_subscript_n_plus_1() {
        // x_{n+1} → x_(n+1) — since '+' has subscript char and n does too
        let result = latex_to_unicode("x_{n+1}");
        assert_eq!(result, "x\u{2099}\u{208A}\u{2081}");
    }

    #[test]
    fn test_superscript_2n() {
        // x^{2n} → x²ⁿ (both 2 and n have superscript chars)
        let result = latex_to_unicode("x^{2n}");
        assert_eq!(result, "x\u{00B2}\u{207F}");
    }

    #[test]
    fn test_combined_expression() {
        // \frac{\hbar^2}{2m}\nabla^2\psi → ℏ²/2m∇²ψ
        let result = latex_to_unicode("\\frac{\\hbar^2}{2m}\\nabla^2\\psi");
        assert!(result.contains("\u{210F}\u{00B2}"));
        assert!(result.contains("/2m"));
        assert!(result.contains("\u{2207}\u{00B2}"));
        assert!(result.contains("\u{03C8}"));
    }

    #[test]
    fn test_hat_accent() {
        let result = latex_to_unicode("\\hat{x}");
        assert_eq!(result, "x\u{0302}");
    }

    #[test]
    fn test_bar_accent() {
        let result = latex_to_unicode("\\bar{z}");
        assert_eq!(result, "z\u{0304}");
    }

    #[test]
    fn test_tilde_accent() {
        let result = latex_to_unicode("\\tilde{x}");
        assert_eq!(result, "x\u{0303}");
    }

    #[test]
    fn test_dot_accent() {
        let result = latex_to_unicode("\\dot{x}");
        assert_eq!(result, "x\u{0307}");
    }

    #[test]
    fn test_delimiter_stripping() {
        // \left( and \right) should be stripped, leaving bare parens
        let result = latex_to_unicode("\\left( x + y \\right)");
        assert!(result.contains("("));
        assert!(result.contains(")"));
        assert!(result.contains("x"));
    }

    #[test]
    fn test_bmatrix() {
        let input = "\\begin{bmatrix} 1 & 0 \\\\ 0 & 1 \\end{bmatrix}";
        assert_eq!(latex_to_unicode(input), "[1 0; 0 1]");
    }

    #[test]
    fn test_vmatrix() {
        let input = "\\begin{vmatrix} a & b \\\\ c & d \\end{vmatrix}";
        assert_eq!(latex_to_unicode(input), "|a b; c d|");
    }

    #[test]
    fn test_operators() {
        assert_eq!(latex_to_unicode("\\sum"), "\u{2211}");
        assert_eq!(latex_to_unicode("\\prod"), "\u{220F}");
        assert_eq!(latex_to_unicode("\\int"), "\u{222B}");
        assert_eq!(latex_to_unicode("\\oint"), "\u{222E}");
        assert_eq!(latex_to_unicode("\\lim"), "lim");
    }

    #[test]
    fn test_operatorname() {
        assert_eq!(latex_to_unicode("\\operatorname{sin}"), "sin");
    }

    #[test]
    fn test_empty_input() {
        assert_eq!(latex_to_unicode(""), "");
    }

    #[test]
    fn test_plain_text() {
        assert_eq!(latex_to_unicode("abc"), "abc");
    }

    #[test]
    fn test_coulomb_law() {
        // F = \frac{1}{4\pi\epsilon_0}\frac{q_1 q_2}{r^2}
        let input = "F = \\frac{1}{4\\pi\\epsilon_0}\\frac{q_1 q_2}{r^2}";
        let result = latex_to_unicode(input);
        assert!(result.contains("F ="));
        assert!(result.contains("\u{03C0}"));
        assert!(result.contains("\u{03B5}"));
        assert!(result.contains("r\u{00B2}"));
    }

    #[test]
    fn test_maxwell_equation() {
        // \nabla \times \vec{E} = -\frac{\partial \vec{B}}{\partial t}
        let input = "\\nabla \\times \\vec{E} = -\\frac{\\partial \\vec{B}}{\\partial t}";
        let result = latex_to_unicode(input);
        assert!(result.contains("\u{2207}"));
        assert!(result.contains("\u{00D7}"));
        assert!(result.contains("\u{2202}"));
    }

    #[test]
    fn test_big_delimiters_stripped() {
        let result = latex_to_unicode("\\Bigg( x \\Bigg)");
        assert!(result.contains("("));
        assert!(result.contains(")"));
        assert!(!result.contains("Bigg"));
    }

    #[test]
    fn test_displaystyle_stripped() {
        let result = latex_to_unicode("\\displaystyle \\frac{a}{b}");
        assert_eq!(result, "a/b");
    }
}
