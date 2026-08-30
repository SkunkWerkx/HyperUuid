import org.gradle.external.javadoc.StandardJavadocDocletOptions

plugins {
    `java-library`
    `maven-publish`
    id("com.vanniktech.maven.publish") version "0.37.0"
}

// io.github.skunkwerkx — the SkunkWerkx org's own auto-verified Central Portal namespace
// (approved by Central Support after an email request; io.github.buvinghausen was the interim
// personal-account namespace used for the very first real Maven Central publish, proving the
// token auth + GPG signing path end to end — that coordinate stays live on Central permanently
// (no delete), this is where every publish from here on happens).
group = "io.github.skunkwerkx"
// The real, committed version — same story as every other binding's manual bump: 0.0.1
// proves this coordinate's own first real Maven Central publish, ahead of the coordinated
// v0.1.0 release. CI overrides this (0.1.0-ci.<run_number>) via HYPERUUID_VERSION for repeated
// manual workflow_dispatch runs against the GitHub Packages feed during testing, so those
// don't collide with an already-published version — the real Maven Central publish
// (release.yml, tag-triggered) never sets that env var, so it always uses this committed
// version as-is.
version = System.getenv("HYPERUUID_VERSION") ?: "0.0.1"

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(platform("org.junit:junit-bom:6.1.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

tasks.test {
    useJUnitPlatform()
    // UuidGenerator's FFM downcalls are a "restricted method" — silences the runtime
    // warning today and avoids them being blocked outright in a future JDK.
    jvmArgs("--enable-native-access=ALL-UNNAMED")
}

java {
    // 22 (not 21, as the prior Kotlin build targeted): java.lang.foreign is a stable, non-
    // preview API only from JDK 22 (JEP 454) onward — plain javac (unlike kotlinc, which
    // doesn't gate on the JDK's own @PreviewFeature markers the same way) enforces that at
    // this project's own source/target level, not just the compiling JDK's.
    sourceCompatibility = JavaVersion.VERSION_22
    targetCompatibility = JavaVersion.VERSION_22
    withSourcesJar()
}

// javadoc's own doclint already flags a missing comment/@param/@return as a WARNING by
// default (that's how the 47 gaps that used to exist on this class's public surface were
// found) — -Xwerror promotes those warnings to build-failing errors, so an undocumented
// public member can't ship again silently. Central Portal requires a javadoc jar for every
// artifact anyway (see mavenPublishing below), so this is enforcing a real publish
// prerequisite, not just style.
tasks.javadoc {
    (options as StandardJavadocDocletOptions).addBooleanOption("Xwerror", true)
}

// mavenPublishing {} (com.vanniktech.maven.publish) owns the "maven" publication itself —
// sources/javadoc jars, POM, and the Central Portal repository target all come from here, not
// from a manually created MavenPublication (that would collide: the plugin creates one named
// "maven" too). publishToMavenCentral() targets the new Central Publisher Portal, not the
// dead OSSRH/Nexus staging API — io.github.skunkwerkx is now an approved, Central-Support-
// verified org namespace (io.github.buvinghausen was the interim personal-account namespace
// that auto-verified on its own, used only for this package's very first real publish).
// Credentials
// (mavenCentralUsername/mavenCentralPassword, from the Central Portal's own token generator —
// not a raw Sonatype account password) and the signing key come from
// ORG_GRADLE_PROJECT_-prefixed env vars in CI, ~/.gradle/gradle.properties locally; neither
// lives in this file.
mavenPublishing {
    publishToMavenCentral()
    signAllPublications()

    pom {
        name.set("hyperuuid")
        description.set(
            "RFC 9562 UUID v4/v5/v6/v7 generation — high-performance, allocation-free " +
                "FFM bindings straight into a native Rust core (libhyperuuid). " +
                "No runtime bridge, no reflection, no extra dependency."
        )
        url.set("https://github.com/SkunkWerkx/HyperUuid")
        licenses {
            license {
                name.set("MIT")
                url.set("https://opensource.org/license/mit")
                distribution.set("repo")
            }
        }
        developers {
            developer {
                id.set("buvinghausen")
                name.set("Brian Buvinghausen")
                url.set("https://github.com/buvinghausen/")
            }
        }
        scm {
            url.set("https://github.com/SkunkWerkx/HyperUuid")
            connection.set("scm:git:git://github.com/SkunkWerkx/HyperUuid.git")
            developerConnection.set("scm:git:ssh://git@github.com/SkunkWerkx/HyperUuid.git")
        }
    }
}

publishing {
    repositories {
        // This repo's GitHub Packages Maven registry (private by default, repo-scoped —
        // github.com/SkunkWerkx/HyperUuid/packages). Credentials come from CI's own
        // GITHUB_ACTOR/GITHUB_TOKEN; empty locally, which only matters if you actually run
        // `./gradlew publish` (publishToMavenLocal doesn't touch this repository). Independent
        // of mavenPublishing {} above — this stays as a second, separate target on the same
        // "maven" publication, not a competing one.
        maven {
            name = "GitHubPackages"
            url = uri("https://maven.pkg.github.com/SkunkWerkx/HyperUuid")
            credentials {
                username = System.getenv("GITHUB_ACTOR")
                password = System.getenv("GITHUB_TOKEN")
            }
        }
    }
}
