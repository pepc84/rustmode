/*
 * Minimal JNI bridge for the ai.serenade.treesitter Java API.
 * Uses the system libtree-sitter and the tree-sitter-rust grammar library.
 *
 * Node objects store a heap-allocated TSNode (24 bytes) in a Java long field
 * named "ptr".  This keeps the JNI code simple at the cost of one malloc per
 * node — acceptable for the small trees RustMode processes.
 */

#include <jni.h>
#include <stdlib.h>
#include <string.h>
#include <tree_sitter/api.h>

/* ── forward declarations ───────────────────────────────────────────────── */
extern TSLanguage *tree_sitter_rust(void);

/* ── helpers ────────────────────────────────────────────────────────────── */

static jfieldID get_ptr_field(JNIEnv *env, jobject obj) {
    jclass cls = (*env)->GetObjectClass(env, obj);
    return (*env)->GetFieldID(env, cls, "ptr", "J");
}

static TSNode *get_node_ptr(JNIEnv *env, jobject obj) {
    jfieldID fid = get_ptr_field(env, obj);
    jlong ptr = (*env)->GetLongField(env, obj, fid);
    return (TSNode *)(uintptr_t)ptr;
}

static jobject make_node(JNIEnv *env, TSNode node) {
    jclass cls = (*env)->FindClass(env, "ai/serenade/treesitter/Node");
    if (!cls) return NULL;
    jmethodID ctor = (*env)->GetMethodID(env, cls, "<init>", "(J)V");
    if (!ctor) return NULL;
    TSNode *heap = malloc(sizeof(TSNode));
    if (!heap) return NULL;
    *heap = node;
    return (*env)->NewObject(env, cls, ctor, (jlong)(uintptr_t)heap);
}

static jobject make_tree(JNIEnv *env, TSTree *tree) {
    jclass cls = (*env)->FindClass(env, "ai/serenade/treesitter/Tree");
    if (!cls) return NULL;
    jmethodID ctor = (*env)->GetMethodID(env, cls, "<init>", "(J)V");
    if (!ctor) return NULL;
    return (*env)->NewObject(env, cls, ctor, (jlong)(uintptr_t)tree);
}

/* ── Languages ──────────────────────────────────────────────────────────── */

JNIEXPORT jlong JNICALL Java_ai_serenade_treesitter_Languages_rust
    (JNIEnv *env, jclass cls) {
    return (jlong)(uintptr_t)tree_sitter_rust();
}

/* ── Parser ─────────────────────────────────────────────────────────────── */

JNIEXPORT jlong JNICALL Java_ai_serenade_treesitter_Parser_createParser
    (JNIEnv *env, jobject obj) {
    return (jlong)(uintptr_t)ts_parser_new();
}

JNIEXPORT void JNICALL Java_ai_serenade_treesitter_Parser_setLanguage
    (JNIEnv *env, jobject obj, jlong lang) {
    jfieldID fid = get_ptr_field(env, obj);
    TSParser *parser = (TSParser *)(uintptr_t)(*env)->GetLongField(env, obj, fid);
    ts_parser_set_language(parser, (TSLanguage *)(uintptr_t)lang);
}

JNIEXPORT jobject JNICALL Java_ai_serenade_treesitter_Parser_parseString
    (JNIEnv *env, jobject obj, jstring src) {
    jfieldID fid = get_ptr_field(env, obj);
    TSParser *parser = (TSParser *)(uintptr_t)(*env)->GetLongField(env, obj, fid);
    const char *str = (*env)->GetStringUTFChars(env, src, NULL);
    jsize len = (*env)->GetStringUTFLength(env, src);
    TSTree *tree = ts_parser_parse_string(parser, NULL, str, (uint32_t)len);
    (*env)->ReleaseStringUTFChars(env, src, str);
    return make_tree(env, tree);
}

JNIEXPORT void JNICALL Java_ai_serenade_treesitter_Parser_destroyParser
    (JNIEnv *env, jobject obj, jlong ptr) {
    ts_parser_delete((TSParser *)(uintptr_t)ptr);
}

/* ── Tree ───────────────────────────────────────────────────────────────── */

JNIEXPORT jobject JNICALL Java_ai_serenade_treesitter_Tree_getRootNode
    (JNIEnv *env, jobject obj) {
    jfieldID fid = get_ptr_field(env, obj);
    TSTree *tree = (TSTree *)(uintptr_t)(*env)->GetLongField(env, obj, fid);
    TSNode root = ts_tree_root_node(tree);
    return make_node(env, root);
}

JNIEXPORT void JNICALL Java_ai_serenade_treesitter_Tree_freeTree
    (JNIEnv *env, jobject obj, jlong ptr) {
    ts_tree_delete((TSTree *)(uintptr_t)ptr);
}

/* ── Node ───────────────────────────────────────────────────────────────── */

JNIEXPORT jint JNICALL Java_ai_serenade_treesitter_Node_getChildCount
    (JNIEnv *env, jobject obj) {
    TSNode *n = get_node_ptr(env, obj);
    return (jint)ts_node_child_count(*n);
}

JNIEXPORT jobject JNICALL Java_ai_serenade_treesitter_Node_getChild
    (JNIEnv *env, jobject obj, jint i) {
    TSNode *n = get_node_ptr(env, obj);
    TSNode child = ts_node_child(*n, (uint32_t)i);
    return make_node(env, child);
}

JNIEXPORT jstring JNICALL Java_ai_serenade_treesitter_Node_getType
    (JNIEnv *env, jobject obj) {
    TSNode *n = get_node_ptr(env, obj);
    const char *type = ts_node_type(*n);
    return (*env)->NewStringUTF(env, type ? type : "");
}

JNIEXPORT jint JNICALL Java_ai_serenade_treesitter_Node_getStartByte
    (JNIEnv *env, jobject obj) {
    TSNode *n = get_node_ptr(env, obj);
    return (jint)ts_node_start_byte(*n);
}

JNIEXPORT jint JNICALL Java_ai_serenade_treesitter_Node_getEndByte
    (JNIEnv *env, jobject obj) {
    TSNode *n = get_node_ptr(env, obj);
    return (jint)ts_node_end_byte(*n);
}

JNIEXPORT jint JNICALL Java_ai_serenade_treesitter_Node_getStartRow
    (JNIEnv *env, jobject obj) {
    TSNode *n = get_node_ptr(env, obj);
    return (jint)ts_node_start_point(*n).row;
}

JNIEXPORT jint JNICALL Java_ai_serenade_treesitter_Node_getStartColumn
    (JNIEnv *env, jobject obj) {
    TSNode *n = get_node_ptr(env, obj);
    return (jint)ts_node_start_point(*n).column;
}

JNIEXPORT jboolean JNICALL Java_ai_serenade_treesitter_Node_hasError
    (JNIEnv *env, jobject obj) {
    TSNode *n = get_node_ptr(env, obj);
    return ts_node_has_error(*n) ? JNI_TRUE : JNI_FALSE;
}
