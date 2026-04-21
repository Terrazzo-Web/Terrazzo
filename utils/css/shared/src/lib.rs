mod parse;

use std::borrow::Cow;

use nameth::NamedEnumValues as _;
use nameth::nameth;
pub use parse::parse_css;

use crate::parse::CssFragment;
use crate::parse::Global;

pub fn rewrite_classes(
    css_file: &str,
    rename: impl Fn(&str) -> String,
) -> Result<String, CssError> {
    let fragments =
        parse::parse_css(css_file).map_err(|error| CssError::ParseError(error.to_string()))?;
    let mut new_file = String::with_capacity(css_file.len() * 2);
    let mut cursor = css_file;

    for fragment in fragments {
        let (span, replace) = match fragment {
            CssFragment::Class(class) => (class, Cow::Owned(rename(class))),
            CssFragment::Global(Global { inner, outer }) => (outer, Cow::Borrowed(inner)),
        };

        let (before, after) = cursor.split_at(span.as_ptr() as usize - cursor.as_ptr() as usize);
        cursor = &after[span.len()..];
        new_file.push_str(before);
        new_file.push_str(&replace);
    }
    new_file.push_str(cursor);

    Ok(new_file)
}

#[nameth]
#[derive(thiserror::Error, Debug)]
pub enum CssError {
    #[error("[{n}] {0}", n = self.name())]
    ParseError(String),
}

#[cfg(test)]
mod tests {

    #[test]
    fn rewrite_classes() {
        let scss = r#"
@charset "utf-8";

// Line comment

/* block comment 
.class {
	{ // Invalid syntax inside the comment to ensure this isn't parsed.
}
*/

div.style1.style2[value="thing"] {
	color: red; // line comment after declaration
	background-color: black;
}

.style-with-dashes {
	color: red;
}

@media (max-width: 600px) {
	.style3 {
		background-color: #87ceeb;
	}
}

@font-face {
	font-family: "Trickster";
	src:
		local("Trickster"),
		url("trickster-COLRv1.otf") format("opentype") tech(color-COLRv1),
		url("trickster-outline.otf") format("opentype"),
		url("trickster-outline.woff") format("woff");
}

div > .style4 {
	.nested-style {
		color: red;
	}

	@media (max-width: 600px) {
		// scss style nested media query with declarations inside
		color: blue;
	}
}

:global(.global-class) {
	color: red;
}

$some-scss-variable: 10px; // Scss variable declarations in top scope.

.style5 // comment in between selector

.style6

/* comment in between selector */
.style7 {
	$some-scss-variable: 10px; // Scss variable declarations inside style rules block.
	color: red;
}

.style1 {
	// Repeated style
	color: blue;
}

// scss placeholder class should parse
%scss-class {
	color: red;
}

@layer test-layer {
	.style8 {
		color: red;
	}
}

@layer test-layer2;

@layer;

@container (min-width: #{$screen-md}) {
	h2 {
		font-size: 1.5em;
	}

	.style9 {
		font-size: 1.5em;
	}
}

//eof
"#;

        let actual = super::rewrite_classes(scss, |c| format!("{c}-REWRITE")).unwrap();
        let expected = r#"
@charset "utf-8";

// Line comment

/* block comment 
.class {
	{ // Invalid syntax inside the comment to ensure this isn't parsed.
}
*/

div.style1-REWRITE.style2-REWRITE[value="thing"] {
	color: red; // line comment after declaration
	background-color: black;
}

.style-with-dashes-REWRITE {
	color: red;
}

@media (max-width: 600px) {
	.style3-REWRITE {
		background-color: #87ceeb;
	}
}

@font-face {
	font-family: "Trickster";
	src:
		local("Trickster"),
		url("trickster-COLRv1.otf") format("opentype") tech(color-COLRv1),
		url("trickster-outline.otf") format("opentype"),
		url("trickster-outline.woff") format("woff");
}

div > .style4-REWRITE {
	.nested-style-REWRITE {
		color: red;
	}

	@media (max-width: 600px) {
		// scss style nested media query with declarations inside
		color: blue;
	}
}

.global-class {
	color: red;
}

$some-scss-variable: 10px; // Scss variable declarations in top scope.

.style5-REWRITE // comment in between selector

.style6-REWRITE

/* comment in between selector */
.style7-REWRITE {
	$some-scss-variable: 10px; // Scss variable declarations inside style rules block.
	color: red;
}

.style1-REWRITE {
	// Repeated style
	color: blue;
}

// scss placeholder class should parse
%scss-class {
	color: red;
}

@layer test-layer {
	.style8-REWRITE {
		color: red;
	}
}

@layer test-layer2;

@layer;

@container (min-width: #{$screen-md}) {
	h2 {
		font-size: 1.5em;
	}

	.style9-REWRITE {
		font-size: 1.5em;
	}
}

//eof
"#;
        assert_eq!(expected, actual);
    }
}
