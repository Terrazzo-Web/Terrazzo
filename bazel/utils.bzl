"""Shared Bazel utilities."""

def make_rules_matrix(single_rule, overrides, **kwargs):
    """Calls a rule once or expands it into named override variants.

    Args:
      single_rule: Rule function to call for the base target or each override.
      overrides: Optional map of target names to per-target argument overrides.
      **kwargs: Base arguments passed to the rule and merged with each override.
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
