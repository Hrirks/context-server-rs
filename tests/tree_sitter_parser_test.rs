//! Phase 3: structural chunking must isolate real declarations for Java, Go,
//! and Dart, and parsing must stay off the async runtime.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use context_server_rs::parser::chunker;
use context_server_rs::parser::chunker::{ChunkKind, SemanticChunk};
use context_server_rs::parser::SourceLanguage;

const JAVA_SOURCE: &str = r#"
package com.acme.svc;

import java.util.List;
import static java.util.Objects.requireNonNull;

public class UserService extends Base implements Runnable {
    private static final int MAX = 10;

    public UserService(String name) { this.name = name; }

    public List<User> fetchAll(String q, int limit) throws IOException {
        return null;
    }

    private void run() {}

    static class Inner {
        void go() {}
    }
}

interface Greeter {
    String greet(String who);
}

enum Color { RED, GREEN }

record Point(int x, int y) {}
"#;

const GO_SOURCE: &str = r#"
package svc

import (
    "fmt"
    "net/http"
)

type User struct {
    Name string
    Age  int
}

type Handler interface {
    Serve(w http.ResponseWriter) error
}

func FetchAll(q string, limit int) ([]User, error) {
    return nil, nil
}

func (s *Server) Serve(w http.ResponseWriter) error {
    return nil
}
"#;

const DART_SOURCE: &str = r#"
import 'dart:async';
import 'package:flutter/material.dart' show Widget;

part 'foo.g.dart';

class UserService extends Base with Mixin implements Runnable {
  final String name;

  UserService(this.name);

  Future<List<User>> fetchAll(String q, {int limit = 10}) async {
    return [];
  }

  void _run() {}
}

mixin Loggable {
  void log() {}
}

enum Color { red, green }

extension StringX on String {
  String get shout => toUpperCase();
}

void main() {
  runApp(MyApp());
}
"#;

fn named<'a>(chunks: &'a [SemanticChunk], name: &str) -> &'a SemanticChunk {
    chunks
        .iter()
        .find(|chunk| chunk.name.as_deref() == Some(name))
        .unwrap_or_else(|| {
            panic!(
                "no chunk named {name}. present: {:?}",
                chunks
                    .iter()
                    .map(|chunk| (chunk.kind, chunk.name.clone()))
                    .collect::<Vec<_>>()
            )
        })
}

/// Import and package chunks carry no name (an import statement is its own
/// identifier), so count by kind rather than by name.
fn count_of(chunks: &[SemanticChunk], kind: ChunkKind) -> usize {
    chunks.iter().filter(|chunk| chunk.kind == kind).count()
}

fn names_of(chunks: &[SemanticChunk], kind: ChunkKind) -> Vec<String> {
    let mut found: Vec<String> = chunks
        .iter()
        .filter(|chunk| chunk.kind == kind)
        .filter_map(|chunk| chunk.name.clone())
        .collect();
    found.sort();
    found
}

// ---------------------------------------------------------------- Java

#[test]
fn java_extracts_package_imports_and_types() {
    let chunks = chunker::chunk_source(SourceLanguage::Java, JAVA_SOURCE).unwrap();

    assert_eq!(names_of(&chunks, ChunkKind::Class), vec!["Inner", "UserService"]);
    assert_eq!(names_of(&chunks, ChunkKind::Interface), vec!["Greeter"]);
    assert_eq!(names_of(&chunks, ChunkKind::Enum), vec!["Color"]);
    assert_eq!(names_of(&chunks, ChunkKind::Record), vec!["Point"]);

    let imports: Vec<&str> = chunks
        .iter()
        .filter(|chunk| chunk.kind == ChunkKind::Import)
        .map(|chunk| chunk.signature.as_str())
        .collect();
    assert!(
        imports.iter().any(|line| line.contains("java.util.List")),
        "expected the List import, got {imports:?}"
    );
    assert!(imports.iter().any(|line| line.contains("static")), "expected the static import");

    assert!(
        chunks.iter().any(|chunk| chunk.kind == ChunkKind::Package),
        "expected a package declaration chunk"
    );
}

#[test]
fn java_signature_has_the_body_stripped_and_whitespace_collapsed() {
    let chunks = chunker::chunk_source(SourceLanguage::Java, JAVA_SOURCE).unwrap();
    let fetch = named(&chunks, "fetchAll");

    assert_eq!(fetch.kind, ChunkKind::Method);
    assert_eq!(
        fetch.signature,
        "public List<User> fetchAll(String q, int limit) throws IOException"
    );
    assert!(
        !fetch.signature.contains('{'),
        "signature must not contain the body: {}",
        fetch.signature
    );
    // The full text keeps the body so a single symbol can be served alone.
    assert!(fetch.text.contains("return null;"));
}

#[test]
fn java_members_record_their_enclosing_container() {
    let chunks = chunker::chunk_source(SourceLanguage::Java, JAVA_SOURCE).unwrap();

    let service_index = chunks
        .iter()
        .position(|chunk| chunk.name.as_deref() == Some("UserService"))
        .unwrap();
    let inner_index = chunks
        .iter()
        .position(|chunk| chunk.name.as_deref() == Some("Inner"))
        .unwrap();

    assert_eq!(named(&chunks, "fetchAll").parent, Some(service_index));
    assert_eq!(named(&chunks, "run").parent, Some(service_index));
    // The nested class is a member of the outer class, and owns go().
    assert_eq!(chunks[inner_index].parent, Some(service_index));
    assert_eq!(named(&chunks, "go").parent, Some(inner_index));
}

#[test]
fn java_container_chunks_serve_only_their_header() {
    let chunks = chunker::chunk_source(SourceLanguage::Java, JAVA_SOURCE).unwrap();
    let service = named(&chunks, "UserService");

    assert_eq!(service.text, service.signature);
    assert!(
        !service.text.contains("fetchAll"),
        "a class chunk should not inline its methods; they are separate chunks"
    );

    // Interfaces are the exception - their member signatures are the payload.
    let greeter = named(&chunks, "Greeter");
    assert!(greeter.text.contains("greet"));
}

#[test]
fn java_line_numbers_are_one_based_and_ordered() {
    let chunks = chunker::chunk_source(SourceLanguage::Java, JAVA_SOURCE).unwrap();
    let fetch = named(&chunks, "fetchAll");

    // Derive the expected line from the source rather than hard-coding it, so
    // the test checks the 1-based mapping instead of a magic number.
    let expected_line = JAVA_SOURCE
        .lines()
        .position(|line| line.contains("fetchAll"))
        .expect("fetchAll is in the fixture")
        + 1;
    assert_eq!(fetch.start_line, expected_line);
    assert!(fetch.end_line > fetch.start_line);
    assert!(fetch.start_byte < fetch.end_byte);
    assert_eq!(
        &JAVA_SOURCE[fetch.start_byte..fetch.end_byte],
        fetch.text.as_str(),
        "byte range and text must agree"
    );
}

// ---------------------------------------------------------------- Go

#[test]
fn go_distinguishes_structs_interfaces_and_functions() {
    let chunks = chunker::chunk_source(SourceLanguage::Go, GO_SOURCE).unwrap();

    assert_eq!(names_of(&chunks, ChunkKind::Struct), vec!["User"]);
    assert_eq!(names_of(&chunks, ChunkKind::Interface), vec!["Handler"]);
    assert_eq!(names_of(&chunks, ChunkKind::Function), vec!["FetchAll"]);
    assert_eq!(names_of(&chunks, ChunkKind::Method), vec!["Serve"]);
}

#[test]
fn go_struct_chunks_keep_their_fields() {
    let chunks = chunker::chunk_source(SourceLanguage::Go, GO_SOURCE).unwrap();
    let user = named(&chunks, "User");

    // A Go struct's fields are its substance, so the body is kept.
    assert!(user.text.contains("Name string"), "got {}", user.text);
    assert!(user.text.contains("Age"), "got {}", user.text);
    // ...but the signature is still just the header.
    assert_eq!(user.signature, "type User struct");
}

#[test]
fn go_imports_are_extracted() {
    let chunks = chunker::chunk_source(SourceLanguage::Go, GO_SOURCE).unwrap();
    assert_eq!(count_of(&chunks, ChunkKind::Import), 1, "one grouped import");
    let declaration = chunks
        .iter()
        .find(|chunk| chunk.kind == ChunkKind::Import)
        .unwrap();
    assert!(declaration.text.contains("net/http"));
}

#[test]
fn go_method_signature_includes_the_receiver() {
    let chunks = chunker::chunk_source(SourceLanguage::Go, GO_SOURCE).unwrap();
    let serve = named(&chunks, "Serve");

    assert!(
        serve.signature.starts_with("func (s *Server) Serve("),
        "got {}",
        serve.signature
    );
}

// ---------------------------------------------------------------- Dart

#[test]
fn dart_extracts_classes_mixins_extensions_and_functions() {
    let chunks = chunker::chunk_source(SourceLanguage::Dart, DART_SOURCE).unwrap();

    assert_eq!(names_of(&chunks, ChunkKind::Class), vec!["UserService"]);
    assert_eq!(names_of(&chunks, ChunkKind::Mixin), vec!["Loggable"]);
    assert_eq!(names_of(&chunks, ChunkKind::Enum), vec!["Color"]);
    assert_eq!(names_of(&chunks, ChunkKind::Extension), vec!["StringX"]);
    assert_eq!(names_of(&chunks, ChunkKind::Function), vec!["main"]);
}

#[test]
fn dart_finds_methods_nested_behind_signature_nodes() {
    let chunks = chunker::chunk_source(SourceLanguage::Dart, DART_SOURCE).unwrap();

    // Dart hides the name two levels down (declaration -> method_signature ->
    // function_signature), which is why find_name walks rather than indexes.
    let mut methods = names_of(&chunks, ChunkKind::Method);
    methods.sort();
    assert_eq!(methods, vec!["_run", "fetchAll", "log", "shout"]);

    let fetch = named(&chunks, "fetchAll");
    assert_eq!(fetch.kind, ChunkKind::Method);
    assert!(
        fetch.signature.contains("Future<List<User>> fetchAll(String q,"),
        "got {}",
        fetch.signature
    );
}

#[test]
fn dart_detects_constructors() {
    let chunks = chunker::chunk_source(SourceLanguage::Dart, DART_SOURCE).unwrap();
    // The class shares the name, and is matched first by a name lookup, so
    // select on kind instead.
    let constructor = chunks
        .iter()
        .find(|chunk| chunk.kind == ChunkKind::Constructor)
        .expect("expected a constructor_signature chunk");

    assert_eq!(constructor.name.as_deref(), Some("UserService"));
}

#[test]
fn dart_imports_and_parts_are_extracted() {
    let chunks = chunker::chunk_source(SourceLanguage::Dart, DART_SOURCE).unwrap();
    // two imports plus the part directive, none of which carry a name
    assert_eq!(count_of(&chunks, ChunkKind::Import), 3);
}

// ---------------------------------------------------------------- offload

/// The core Phase 3 requirement: parsing is CPU-bound and must run on the
/// blocking pool. On a current_thread runtime the runtime thread and the
/// blocking-pool thread are observably different, so assert on thread identity
/// rather than trusting that spawn_blocking was called.
#[tokio::test(flavor = "current_thread")]
async fn parsing_is_offloaded_off_the_runtime_thread() {
    let runtime_thread = std::thread::current().id();

    let parse_thread = chunker::offload(|| Ok(std::thread::current().id()))
        .await
        .unwrap();

    assert_ne!(
        runtime_thread, parse_thread,
        "offload ran on the async runtime thread - a large parse would stall the MCP server"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn async_chunking_returns_the_same_result_as_the_sync_path() {
    let sync_chunks = chunker::chunk_source(SourceLanguage::Java, JAVA_SOURCE).unwrap();
    let async_chunks = chunker::chunk_source_async(SourceLanguage::Java, JAVA_SOURCE.to_string())
        .await
        .unwrap();

    assert_eq!(sync_chunks.len(), async_chunks.len());
    assert_eq!(sync_chunks[0].signature, async_chunks[0].signature);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn many_parses_run_concurrently_without_deadlock() {
    let completed = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    for _ in 0..32 {
        let counter = Arc::clone(&completed);
        handles.push(tokio::spawn(async move {
            let chunks =
                chunker::chunk_source_async(SourceLanguage::Go, GO_SOURCE.to_string())
                    .await
                    .unwrap();
            assert!(!chunks.is_empty());
            counter.fetch_add(1, Ordering::SeqCst);
        }));
    }

    for handle in handles {
        handle.await.unwrap();
    }
    assert_eq!(completed.load(Ordering::SeqCst), 32);
}

// ---------------------------------------------------------------- discovery

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn discovery_skips_build_and_dependency_directories() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("App.java"), "class App {}").unwrap();
    std::fs::write(root.join("main.go"), "package main").unwrap();
    std::fs::write(root.join("README.md"), "not source").unwrap();

    for skipped in ["target", "node_modules"] {
        let nested = root.join(skipped).join("deep");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("Ignored.java"), "class Ignored {}").unwrap();
    }

    let discovered = chunker::discover_sources(root).unwrap();
    let names: Vec<String> = discovered
        .iter()
        .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
        .collect();

    assert!(names.contains(&"App.java".to_string()));
    assert!(names.contains(&"main.go".to_string()));
    assert!(!names.contains(&"README.md".to_string()));
    assert!(
        !names.contains(&"Ignored.java".to_string()),
        "build and dependency directories must be skipped, got {names:?}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn directory_chunking_merges_every_supported_file() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();

    std::fs::write(root.join("A.java"), "class A { void a() {} }").unwrap();
    std::fs::write(root.join("B.dart"), "class B { void b() {} }").unwrap();
    std::fs::write(root.join("C.go"), "package c\nfunc C() {}").unwrap();

    let chunks = chunker::chunk_directory_async(root.to_path_buf()).await.unwrap();

    let all: Vec<String> = chunks.iter().filter_map(|c| c.name.clone()).collect();
    for expected in ["A", "a", "B", "b", "C"] {
        assert!(
            all.contains(&expected.to_string()),
            "missing {expected} in {all:?}"
        );
    }
}
