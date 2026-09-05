/*
 * C ABI for the Concurrent-C StyleBench engine, for embedding (Ladybird seam).
 *
 * Structured ingest only — no CSS text on the hot path. Rules are compounds +
 * combinators + longhand decls; the tree is upserted by host style-node id.
 * Matching fills stylecc_take_matches; cascade/StyRefs still run inside the
 * engine but Ladybird's C++ StyleComputer is the layout-facing cascade.
 */
#ifndef STYLECC_H
#define STYLECC_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct StyleCC StyleCC;

/* Origins match the engine: UA=0, author=1, style attribute=2. */
enum {
    STYLECC_ORIGIN_UA = 0,
    STYLECC_ORIGIN_AUTHOR = 1,
    STYLECC_ORIGIN_STYLE = 2
};

/* Combinators between compounds (subject is last). */
enum {
    STYLECC_COMB_DESC = 0,
    STYLECC_COMB_CHILD = 1,
    STYLECC_COMB_NEXT = 2,
    STYLECC_COMB_SUBSEQ = 3
};

/* Pseudo-element target on a rule / match. */
enum {
    STYLECC_PE_NONE = 0,
    STYLECC_PE_BEFORE = 1,
    STYLECC_PE_AFTER = 2
};

/* Structural / nth position kinds (engine PosPred). */
enum {
    STYLECC_POS_CHILD = 0,
    STYLECC_POS_LAST_CHILD = 1,
    STYLECC_POS_OF_TYPE = 2,
    STYLECC_POS_LAST_OF_TYPE = 3,
    STYLECC_POS_ONLY_OF_TYPE = 4,
    STYLECC_POS_EMPTY = 5
};

/* Feature delta kinds for stylecc_apply_feature. */
enum {
    STYLECC_FEAT_TAG = 0,
    STYLECC_FEAT_ID = 1,
    STYLECC_FEAT_CLASS = 2,
    STYLECC_FEAT_ATTR = 3
};

typedef struct StyleCCCompound {
    uint32_t type_atom; /* 0 = universal / absent */
    uint32_t id_atom;
    uint32_t const* class_atoms;
    size_t nclass;
    uint32_t const* attr_name_atoms;
    uint32_t const* attr_value_atoms; /* 0 = presence-only */
    uint8_t const* attr_equals;      /* 1 = [name=value], 0 = [name] */
    size_t nattr;
    uint32_t pos_mask; /* bit i set => PosPred row i must hold */
    uint8_t pelem;     /* STYLECC_PE_* on this compound (usually subject only) */
} StyleCCCompound;

typedef struct StyleCCDecl {
    char const* prop; /* longhand name, e.g. "background-color" */
    size_t prop_len;
    char const* value;
    size_t value_len;
    uint8_t important;
} StyleCCDecl;

typedef struct StyleCCMatch {
    uint32_t host_node; /* host style-node id */
    int rule_id;        /* stylecc rule id (0-based) */
    uint8_t pelem;      /* STYLECC_PE_* */
} StyleCCMatch;

StyleCC* stylecc_create(void);
void stylecc_destroy(StyleCC* eng);

/* Intern UTF-8; returns non-zero atom id. Atom 0 is reserved / empty. */
uint32_t stylecc_intern(StyleCC* eng, char const* s, size_t len);

/* Ensure host style-node `host_id` exists; create empty if missing. */
int stylecc_ensure_node(StyleCC* eng, uint32_t host_id);

/* Upsert element facts. parent_host=UINT32_MAX => root. classes/attrs replaced. */
int stylecc_upsert_node(StyleCC* eng, uint32_t host_id, uint32_t parent_host,
                        uint32_t type_atom, uint32_t id_atom,
                        uint32_t const* class_atoms, size_t nclass,
                        uint32_t const* attr_name_atoms, uint32_t const* attr_value_atoms,
                        size_t nattr);

/* Connect / disconnect / reparent. parent_host=UINT32_MAX when disconnecting. */
int stylecc_set_connected(StyleCC* eng, uint32_t host_id, int connected,
                          uint32_t parent_host, int sibling_index);

/* Class / attr / id / tag feature change. present=0 removes. */
int stylecc_apply_feature(StyleCC* eng, uint32_t host_id, int feature_kind,
                          uint32_t name_atom, uint32_t value_atom, int present);

/* Viewport width in CSS px (media queries). */
void stylecc_set_viewport_width(StyleCC* eng, int width_px);

/* Register a position predicate row; returns row index or -1. */
int stylecc_add_pos_pred(StyleCC* eng, int kind, int a, int b);

/*
 * Add a rule. compounds[0..n-1] left-to-right; combs[i] joins compounds[i] to
 * compounds[i+1] (n-1 entries). Returns rule id (>=0) or -1.
 */
int stylecc_add_rule(StyleCC* eng, StyleCCCompound const* compounds, size_t ncomp,
                     uint8_t const* combs, int origin, int mq_row, uint8_t pelem);

/* Replace declaration block (longhand name+value text). */
int stylecc_set_rule_decls(StyleCC* eng, int rule_id, StyleCCDecl const* decls, size_t ndecl);

/* Host rule id map: Ladybird numbers rules 1-based; we store the host id. */
void stylecc_set_rule_host_id(StyleCC* eng, int rule_id, uint32_t host_rule_id);
uint32_t stylecc_rule_host_id(StyleCC* eng, int rule_id);

/* After sheet mutations: rebuild inverted indexes. Call before style_*. */
int stylecc_index_rules(StyleCC* eng);

int stylecc_style_all(StyleCC* eng);

/* paint_blooms → fanout → media → reset → style_dirty → clear. */
int stylecc_style_dirty(StyleCC* eng);

/* Mark host node dirty (bypass fanout). */
void stylecc_mark_dirty(StyleCC* eng, uint32_t host_id);

/* Matches from the last style_all / style_dirty (per live element + pelem). */
size_t stylecc_match_count(StyleCC* eng);
size_t stylecc_take_matches(StyleCC* eng, StyleCCMatch* out, size_t cap);

/* Dirty host nodes from the last style_dirty (or all after style_all). */
size_t stylecc_dirty_count(StyleCC* eng);
size_t stylecc_take_dirty(StyleCC* eng, uint32_t* out, size_t cap);

/* Drop counts for the host adapter (unsupported selectors, etc.). */
void stylecc_note_drop(StyleCC* eng, char const* reason);
uint64_t stylecc_drop_count(StyleCC* eng);

/* Fixture path for standalone ABI tests (not used by Ladybird). */
int stylecc_load_fixture(StyleCC* eng, char const* text, size_t len);

#ifdef __cplusplus
}
#endif

#endif /* STYLECC_H */
