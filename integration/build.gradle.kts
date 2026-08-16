// Hyperscope JVM integration — vendored spark Minestom platform glue (me.lucko.spark.minestom.*),
// consumed by the Stomzy build as the `:hyperscope` subproject (Stomzy/settings.gradle.kts wires
// projectDir = ../Hyperscope/integration — same pattern as :polar). Shared by :demo and every
// games/* backend.
//
// Previously lived at Stomzy/modules/spark-minestom (and before that inside :demo/src, with every
// downstream game declaring the external `dev.lu15:spark-minestom:1.10-SNAPSHOT` artifact — a
// third-party repackaging hosted on a flaky mirror). This module is the ONE vendored copy.
//
// NOTE: this module only builds as part of the Stomzy Gradle workspace (the `minestom.java-library`
// precompiled plugin and `project(":")` resolve there) — it is inert to this repo's Cargo build.
// Future Terrorify-owned metrics tools land here under dev.terrorify.hyperscope (see README.md).

plugins {
    id("minestom.java-library")
}

dependencies {
    implementation(project(":")) // engine core — CommandSender/ConsoleSender

    if (findProject(":hyperscope-common") != null) {
        api(project(":hyperscope-common"))
        api(project(":hyperscope-api"))
    } else {
        api(files("libs/spark-common-adventure5.jar", "libs/spark-api-adventure5.jar"))
    }
    api("net.kyori:adventure-api:5.1.0")
    api("net.kyori:adventure-text-serializer-gson:5.1.0")
    api("net.kyori:adventure-text-serializer-legacy:5.1.0")
    implementation("tools.profiler:async-profiler:4.5")
    implementation("org.ow2.asm:asm:9.7")
    implementation("net.bytebuddy:byte-buddy-agent:1.14.17")
    implementation("com.google.protobuf:protobuf-javalite:4.31.1")
    implementation("me.lucko:bytesocks-java-client:1.0-SNAPSHOT")
    implementation("net.kyori:adventure-text-feature-pagination:4.0.0-SNAPSHOT")
    api("com.google.guava:guava:33.3.1-jre")
}

repositories {
    maven("https://repo.lucko.me/")
}
