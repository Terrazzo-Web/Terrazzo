"""Shared Bazel utilities."""

def make_rules_matrix(single_rule, overrides, **kwargs):
    """Rust rules bundle

    Args:
      single_rule: Map of parameters to generate a matrix of rust_rules
      overrides: Map of parameters to generate a matrix of rust_rules
      **kwargs: Additional arguments
    """
    if overrides == None:
        single_rule(**kwargs)
        return

    for override_name, override_values in overrides.items():
        kwargs_override = _merge_dicts_concat_lists(kwargs, override_values)
        single_rule(name = override_name, **kwargs_override)

def _merge_dicts_concat_lists(a, b):
    result = dict(a)
    for k, v_b in b.items():
        if k in a:
            v_a = a[k]
            if type(v_a) == "list" and type(v_b) == "list":
                result[k] = v_a + v_b
            else:
                fail("Mismatch {} != {}".format(v_a, v_b))
        else:
            result[k] = v_b
    return result
