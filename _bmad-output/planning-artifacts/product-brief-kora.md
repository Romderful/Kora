---
title: "Product Brief: Kora"
status: "complete"
created: "2026-04-04"
updated: "2026-04-04"
inputs:
  - product discovery conversations
  - existing data-plane integration analysis
  - Confluent Schema Registry API documentation
---

# Product Brief: Kora

## Executive Summary

Schema registries are essential infrastructure in every Kafka-based data platform — they ensure producers and consumers agree on the shape of the data flowing through the system. Today, teams running self-hosted Kafka face a frustrating choice: the Confluent Schema Registry (proprietary, JVM-heavy, tightly coupled to the Confluent ecosystem) or open-source alternatives that are Python-based, slow under load, and store schemas back into Kafka topics — creating a circular dependency.

Kora is a schema registry written in Rust, storing schemas in PostgreSQL, and implementing 100% of the Confluent Schema Registry API. It delivers the performance characteristics of a compiled language, the operational simplicity of a database teams already run, and full compatibility with every tool in the Kafka ecosystem — Debezium, Kafka Connect, ksqlDB, and all Confluent serializers. Beyond API compatibility, Kora adds first-class schema comparison capabilities, making it possible to understand exactly how schemas evolve over time.

Kora is built to serve the data engineering community as a lightweight, high-performance alternative to existing registries, with a first production deployment planned inside a battle-tested CDC pipeline platform.

## The Problem

Every team running Kafka needs a schema registry. The current landscape forces painful trade-offs:

- **Confluent Schema Registry** is proprietary, requires the JVM, and pulls teams toward the full Confluent platform. For teams using Redpanda, Tansu, or vanilla Kafka, it's an awkward dependency.
- **Open-source alternatives** are implemented in Python. Under production load with thousands of schemas, latency degrades. They store schemas in Kafka's `_schemas` topic — meaning your schema registry depends on your Kafka cluster being healthy, the very thing it's supposed to help validate against.
- **Schema comparison is missing.** No existing solution offers a way to programmatically diff two schema versions and understand what changed — field additions, type changes, breaking modifications. Teams resort to manual inspection or custom tooling.
- **Operational overhead.** Existing solutions introduce additional infrastructure dependencies (JVM runtime or Python environment) that data teams must maintain alongside their already complex Kafka deployments.

Data engineers running CDC pipelines and streaming platforms experience these pain points daily — registry performance impacts pipeline throughput, maintenance burden consumes engineering time, and the lack of schema comparison features forces manual workarounds when handling constant schema evolution.

## The Solution

Kora is a Confluent Schema Registry API-compatible service that makes three deliberate architectural choices:

1. **Rust** — Single binary, minimal resource footprint, predictable latency under load. No JVM, no Python runtime, no garbage collection pauses.

2. **PostgreSQL storage** — Schemas live in a real database, not a Kafka topic. This is more than a storage choice — it's a platform decision. PG means SQL queries over your schema catalog, standard backup/restore with pg_dump, audit trails, row-level security for multi-tenancy, and no circular dependency on Kafka. Your schemas become first-class database citizens managed with tools your team already knows.

3. **Schema comparison** — First-class API for diffing schema versions. Understand exactly what changed between versions: added fields, removed fields, type changes, compatibility impact — programmatically, not manually. This unlocks schema-aware CI/CD gates, PR-level compatibility checks, and clear visibility into schema evolution.

The Confluent Schema Registry REST API is implemented in full: subjects, versions, schemas by ID, compatibility checking, global and per-subject configuration — supporting Avro, JSON Schema, and Protobuf formats.

## What Makes This Different

| | Confluent SR | OSS Alternatives | Kora |
|---|---|---|---|
| Language | Java (JVM) | Python | Rust |
| Storage | Kafka topic | Kafka topic | PostgreSQL |
| License | Confluent Community | Apache 2.0 | Open source |
| Schema comparison | No | No | Yes |
| Memory footprint | High | Moderate | Low |
| Kafka dependency | Circular | Circular | None |
| Operational complexity | JVM tuning | Python env | Single binary |
| Queryable schema catalog | No | No | Yes (SQL) |

The real differentiator is the architectural bet: **schemas belong in a database, not a Kafka topic**. Combined with Rust performance, full Confluent compatibility, and schema comparison — no existing solution offers this stack.

## Who This Serves

**Primary: Data engineers and platform teams running self-hosted Kafka**
Teams using Kafka (vanilla, Strimzi, Redpanda, Tansu) who need a schema registry that's lightweight, fast, and doesn't add operational complexity. Particularly relevant for the Redpanda ecosystem — Redpanda has no first-party schema registry and recommends third-party solutions. Kora + Redpanda is the complete Confluent-free data streaming stack.

**Secondary: Data platform builders**
Companies building products on top of Kafka that embed a schema registry as a component. They need something embeddable, performant, and with PG storage that fits their existing database infrastructure — not a separate Kafka topic dependency.

## Success Criteria

- **Drop-in compatibility:** Passes the Confluent Schema Registry API compatibility test suite. Any tool that works with Confluent SR works with Kora without configuration changes.
- **Performance:** Sub-millisecond schema lookups, 10x improvement over Python-based registries on registration throughput under concurrent load.
- **Production integration:** Successfully replaces an existing schema registry in a production CDC data-plane with zero application code changes.
- **Community adoption:** GitHub stars, Docker pulls, and usage reports from self-hosted Kafka operators.

## Scope — V1

**In scope:**
- Full Confluent Schema Registry REST API (all endpoints)
- Avro, JSON Schema, and Protobuf format support
- All compatibility modes (BACKWARD, FORWARD, FULL, NONE + TRANSITIVE variants)
- PostgreSQL storage backend
- Schema comparison API (diff between versions)
- Docker image and single binary distribution
- Basic authentication support
- Health check endpoints

**Out of scope for V1:**
- Kafka REST Proxy (separate concern — not needed)
- HA leader election (single instance first, PG handles durability)
- Web UI for schema browsing
- Schema normalization for Protobuf
- OAuth2/OIDC authentication

## Vision

If Kora succeeds, it becomes the default schema registry for the non-Confluent Kafka ecosystem. The PostgreSQL storage opens doors that Kafka topic storage never could: SQL queries over schemas, cross-registry federation, integration with existing data catalogs, and schema governance workflows built on familiar database tooling.

In 2-3 years, Kora is what teams reach for when they set up Kafka — the way they reach for PostgreSQL itself: reliable, fast, boring infrastructure that just works. The PostgreSQL foundation opens paths that Kafka-topic storage structurally cannot: cross-registry schema federation, integration with data catalogs, and schema governance workflows built on familiar database tooling.
