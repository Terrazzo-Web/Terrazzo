# Utils to resolve the list of dependencies that are pulled by a feature

## Chapter 1. Format of the Cargo.toml file

We are only interested in the `[features]` section.

Each feature "F" pulls a list of dependencies that can have various formats:
    (a) If the string starts with "dep:", it means feature "F" triggers an additional compile-time dependency
    (b) If the string contains a slash, it means feature "F" activates another feature on a dependency
    (c) Otherwise, the string points to another feature "F2" that is automatically pulled in by the feature "F"

## Chapter 2. Command line tool

A Rust executable that takes 2 parameters:
    - the path to an existing Cargo.toml file
    - the path to a .bzl file to be created

Parsing of command-line parameters should be done with Rust clap crate.

The command line tool should
    1. Parse the `[features]` section of the Cargo.toml file.
    2. Keep a hashset of features that have already been visited.
    3. Follow instructions from Chapter 3.

## Chapter 3. Algorithm

For each feature, traverse the list of strings, referring to cases from Chapter 1:
    - Case (a): record that there is an extra dependency in a "dependencies" sorted set. Strip out the "dep:"
    - Case (b): ignore
    - Case (c): recursively process that feature first. Record that feature name in a "child_features" sorted set
       
Then print a constant that is the upper-case name of the feature.

The value of the constant is:
- The sum of the "child_features" upper-case names. Because the algorithm is DFS, the constants for the child features will be printed before
- Plus the list of strings in the "dependencies" set

## Chapter 4. Packaging

Create Bazel rules
    1. A Bazel rule to compile the Command line tool from Chapter 2. The name of the crate is "feature-deps".
    2. A Bazel rule generates the .bzl file. It's a custom Bazel rule with multiple instructions
        - parameters are name and path
        - the name parameter defaults to the current package name (I think it's the same as the basename of the current directory), no slashes in that default name.
        - path parameter points to a Cargo.toml file, and which defaults to the "Cargo.toml" file in the current folder
        - the rule calls the feature-deps binary and generates a "generated.{name}-features.bzl" file
        - the rule then calls the generate_file rule to copy the generated "generated.{name}-features.bzl" into checked-in "{name}-features.bzl" file

As an example, apply this to terminal/Cargo.toml
