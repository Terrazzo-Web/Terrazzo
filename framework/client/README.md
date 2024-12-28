# Terrazzo client

Template library to generate dynamic HTML documents.

# Usage

The template library is usually used in tandem with the `terrazzo-macro` crate.

## The `#[html]` macro

### Basic usage
This macro us used to generate dynamic HTML nodes.

```
use terrazzo_client::prelude::*;
use terrazzo_macro::html;

#[html]
fn sample() -> XElement {
    div(
        h1("Section 1"),
        ul(li("Firstly"), li("Secondly")),
        h1("Section 2"),
        ol(li("One"), li("One"), li("One")),
    )
}
```

### List of nodes
List of nodes can be generated from iterators

```
# use terrazzo_client::prelude::*;
# use terrazzo_macro::html;
#[html]
fn sample() -> XElement {
    let list = [1, 2, 3].map(|i| li("{i}"));
    div(h1("Title"), ul(list..))
}
```

This function generates:
```html
<div>
    <h1> Title </h1>
    <ul>
        <li> 1 </li>
        <li> 2 </li>
        <li> 3 </li>
    </ul>
</div>
```

### Attributes
```
# use terrazzo_client::prelude::*;
# use terrazzo_macro::html;
#[html]
fn sample() -> XElement {
    div(
        class = "my-css-class",
        style = format!("width: {}%", 100),
        "Content",
    )
}
```

### Optional attributes
This can be useful when a function generates a node and an attribute may or may not have a value.
```
# use terrazzo_client::prelude::*;
# use terrazzo_macro::html;
#[html]
fn sample(class: Option<String>) -> XElement {
    div(
        class |= class,
        style = format!("width: {}%", 100),
        "Content",
    )
}
```

### Style properties
Style properties can be set individually.
```
# use terrazzo_client::prelude::*;
# use terrazzo_macro::html;
#[html]
fn sample(width: i32) -> XElement {
    div(
        style::width = format!("{}%", width),
        "Content",
    )
}
```
