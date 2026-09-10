/*
 * Smoke test for libstylecc: structured tree + one rule + style + matches.
 * Build: scripts/test-stylecc-abi.sh
 */
#include "stylecc.h"
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static void die(char const* msg) {
    fprintf(stderr, "FAIL: %s\n", msg);
    exit(1);
}

int main(void) {
    StyleCC* e = stylecc_create();
    if (!e)
        die("stylecc_create");

    uint32_t a_div = stylecc_intern(e, "div", 3);
    uint32_t a_span = stylecc_intern(e, "span", 4);
    uint32_t a_foo = stylecc_intern(e, "foo", 3);
    uint32_t a_bg = stylecc_intern(e, "background-color", 16);
    (void)a_bg;
    if (!a_div || !a_span || !a_foo)
        die("intern");

    /* host ids 1=root div, 2=span.foo child */
    if (stylecc_upsert_node(e, 1, UINT32_MAX, a_div, 0, NULL, 0, NULL, NULL, 0) < 0)
        die("upsert root");
    uint32_t classes[] = { a_foo };
    if (stylecc_upsert_node(e, 2, 1, a_span, 0, classes, 1, NULL, NULL, 0) < 0)
        die("upsert child");

    /* rule: span.foo { background-color: red } */
    StyleCCCompound comp = {
        .type_atom = a_span,
        .id_atom = 0,
        .class_atoms = classes,
        .nclass = 1,
        .attr_name_atoms = NULL,
        .attr_value_atoms = NULL,
        .attr_equals = NULL,
        .nattr = 0,
        .pos_mask = 0,
        .pelem = STYLECC_PE_NONE,
    };
    int rid = stylecc_add_rule(e, &comp, 1, NULL, STYLECC_ORIGIN_AUTHOR, -1, STYLECC_PE_NONE);
    if (rid < 0)
        die("add_rule");
    stylecc_set_rule_host_id(e, rid, 42);
    StyleCCDecl decl = {
        .prop = "background-color",
        .prop_len = 16,
        .value = "red",
        .value_len = 3,
        .important = 0,
    };
    if (stylecc_set_rule_decls(e, rid, &decl, 1) != 0)
        die("set_rule_decls");

    if (stylecc_index_rules(e) != 0)
        die("index_rules");
    if (stylecc_style_all(e) != 0)
        die("style_all");

    size_t nm = stylecc_match_count(e);
    if (nm == 0)
        die("expected at least one match for span.foo");
    StyleCCMatch* matches = calloc(nm, sizeof(*matches));
    if (!matches)
        die("oom");
    size_t got = stylecc_take_matches(e, matches, nm);
    if (got != nm)
        die("take_matches count");
    int hit = 0;
    for (size_t i = 0; i < nm; i++) {
        if (matches[i].host_node == 2 && matches[i].rule_id == rid)
            hit = 1;
    }
    free(matches);
    if (!hit)
        die("match did not name host 2 / our rule");

    if (!stylecc_node_restyled(e, 2))
        die("span not restyled");
    {
        size_t hn = stylecc_node_match_count(e, 2);
        if (!hn)
            die("host consume view empty");
        StyleCCMatch* hostm = calloc(hn, sizeof(*hostm));
        if (!hostm)
            die("oom hostm");
        stylecc_consume_node_matches(e, 2, hostm, hn);
        hit = 0;
        for (size_t i = 0; i < hn; i++) {
            if (hostm[i].host_node == 2 && hostm[i].rule_id == 42)
                hit = 1;
        }
        free(hostm);
        if (!hit)
            die("consume view did not map host rule 42");
    }
    {
        StyleCCUsed used;
        if (stylecc_consume_node_used(e, 2, &used) != 0)
            die("consume used");
        if (used.host_node != 2)
            die("used host");
        if (used.r != 255 || used.g != 0 || used.b != 0)
            die("used bg not red");
        if (used.display != 0)
            die("span used display not inline");
        if (!used.height_auto)
            die("span height not auto");
        if (stylecc_used_count(e) == 0)
            die("used_count after style_all");
    }

    /* class remove → dirty restyle → no match */
    if (stylecc_apply_feature(e, 2, STYLECC_FEAT_CLASS, a_foo, 0, 0) != 0)
        die("remove class");
    if (stylecc_style_dirty(e) != 0)
        die("style_dirty");
    nm = stylecc_match_count(e);
    matches = calloc(nm ? nm : 1, sizeof(*matches));
    stylecc_take_matches(e, matches, nm ? nm : 1);
    hit = 0;
    for (size_t i = 0; i < nm; i++) {
        if (matches[i].host_node == 2 && matches[i].rule_id == rid)
            hit = 1;
    }
    free(matches);
    if (hit)
        die("still matched after class remove");
    if (!stylecc_node_restyled(e, 2))
        die("span not restyled after class remove");
    if (stylecc_node_match_count(e, 2) != 0)
        die("host view still had matches after class remove");

    /* fixture path still works (same section grammar as make compare) */
    char const* fixture =
        "---base---\n"
        "#testroot { font-size: 10px; }\n"
        "---css---\n"
        ".x { color: blue; }\n"
        "---tree---\n"
        "0\t-1\tdiv\ttestroot\t\t\n"
        "1\t0\tspan\t-\tx\t\n";
    StyleCC* e2 = stylecc_create();
    if (!e2)
        die("create2");
    if (stylecc_load_fixture(e2, fixture, strlen(fixture)) != 0)
        die("load_fixture");
    if (stylecc_style_all(e2) != 0)
        die("fixture style_all");
    {
        StyleCCUsed root_used, child_used;
        if (stylecc_consume_node_used(e2, 1, &root_used) != 0)
            die("fixture root used");
        if (stylecc_consume_node_used(e2, 2, &child_used) != 0)
            die("fixture child used");
        if (root_used.font_size_px < 9.5f || root_used.font_size_px > 10.5f)
            die("fixture #testroot font-size not 10px");
        if (child_used.font_size_px < 9.5f || child_used.font_size_px > 10.5f)
            die("fixture child did not inherit font-size 10px");
        if (child_used.display != 0)
            die("fixture child display not packed inline");
    }

    /* StyleBench #testroot * { display: inline-block } must pack as 2. */
    {
        char const* ib =
            "---base---\n"
            "#testroot * { display: inline-block; }\n"
            "---css---\n"
            ".x { color: blue; }\n"
            "---tree---\n"
            "0\t-1\tdiv\ttestroot\t\t\n"
            "1\t0\tspan\t-\tx\t\n";
        StyleCC* e3 = stylecc_create();
        if (!e3)
            die("create3");
        if (stylecc_load_fixture(e3, ib, strlen(ib)) != 0)
            die("load_fixture inline-block");
        if (stylecc_style_all(e3) != 0)
            die("inline-block style_all");
        StyleCCUsed child_ib;
        if (stylecc_consume_node_used(e3, 2, &child_ib) != 0)
            die("inline-block child used");
        if (child_ib.display != 2)
            die("fixture child display not packed inline-block");
        stylecc_destroy(e3);
    }
    stylecc_destroy(e2);

    stylecc_destroy(e);
    printf("OK stylecc abi smoke\n");
    return 0;
}
