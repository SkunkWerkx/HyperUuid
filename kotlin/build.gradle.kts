plugins {
    kotlin("jvm") version "2.4.10"
    `java-library`
    `maven-publish`
}

group = "io.github.buvinghausen"
// CI overrides this (0.1.0-ci.<run_number>) via HYPERUUID_VERSION so repeated manual
// workflow_dispatch runs during testing don't collide with an already-published version on
// the GitHub Packages feed. Real releases (tag push) still need a deliberate bump here.
version = System.getenv("HYPERUUID_VERSION") ?: "0.1.0"

repositories {
    mavenCentral()
}

dependencies {
    testImplementation(platform("org.junit:junit-bom:6.1.3"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

kotlin {
    // Target JDK 21 bytecode without pinning a toolchain — avoids Gradle needing to
    // auto-provision a JDK when a newer one is already what's running the build.
    compilerOptions {
        jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_21)
    }
}

tasks.test {
    useJUnitPlatform()
    // UuidGenerator's FFM downcalls are a "restricted method" — silences the runtime
    // warning today and avoids them being blocked outright in a future JDK.
    jvmArgs("--enable-native-access=ALL-UNNAMED")
}

java {
    sourceCompatibility = JavaVersion.VERSION_21
    targetCompatibility = JavaVersion.VERSION_21
    withSourcesJar()
}

publishing {
    publications {
        create<MavenPublication>("maven") {
            from(components["java"])
            pom {
                name.set("hyperuuid")
                description.set(
                    "RFC 9562 UUID v4/v5/v7 generation — high-performance, allocation-free " +
                        "FFM bindings straight into a native Rust core (libhyperuuid). " +
                        "No runtime bridge, no reflection."
                )
                url.set("https://github.com/SkunkWerkx/HyperUuid")
                licenses {
                    license {
                        name.set("MIT")
                    }
                }
            }
        }
    }
    repositories {
        // This repo's GitHub Packages Maven registry (private by default, repo-scoped —
        // github.com/SkunkWerkx/HyperUuid/packages). Credentials come from CI's own
        // GITHUB_ACTOR/GITHUB_TOKEN; empty locally, which only matters if you actually run
        // `./gradlew publish` (publishToMavenLocal doesn't touch this repository).
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
