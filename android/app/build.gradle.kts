plugins {
    alias(libs.plugins.android.application)
    alias(libs.plugins.kotlin.compose)
}

abstract class RustlsVersion : ValueSource<String, RustlsVersion.Params> {
    interface Params : ValueSourceParameters {
        val lockFile: RegularFileProperty
    }

    companion object {
        const val CRATE_NAME = "rustls-platform-verifier-android"
    }

    override fun obtain(): String {
        val lockFile = parameters.lockFile.get().asFile
        val version = lockFile.readLines().let { lines ->
            val nameIndex = lines.indexOfFirst { it.trim() == "name = \"$CRATE_NAME\"" }
            if (nameIndex < 0) {
                null
            } else {
                lines.drop(nameIndex + 1)
                    .firstOrNull { it.trimStart().startsWith("version = ") }
                    ?.substringAfter('"', "")
                    ?.substringBefore('"', "")
                    ?.takeIf { it.isNotEmpty() }
            }
        }
        return version ?: error("$CRATE_NAME not found in $lockFile")
    }
}

val rustlsPlatformVerifierVersion = providers.of(RustlsVersion::class.java) {
    parameters.lockFile.set(layout.projectDirectory.file("../../Cargo.lock"))
}

configurations.configureEach {
    resolutionStrategy.eachDependency {
        if (requested.group == "org.rustls" && requested.name == "rustls-platform-verifier") {
            useVersion(rustlsPlatformVerifierVersion.get())
            because("native component version must match ${RustlsVersion.CRATE_NAME}")
        }
    }
}

val rustWorkspaceDir = rootDir.parentFile
val rustTargetDir = rustWorkspaceDir.resolve("target")
val rustInputs = fileTree(rustWorkspaceDir) {
    include(
        "Cargo.toml",
        "Cargo.lock",
        "crates/**/*.rs",
        "crates/**/Cargo.toml",
        "crates/**/uniffi.toml",
        "assets/cleanurls_rules.json",
    )
}
val uniffiOutputDir = layout.buildDirectory.dir("generated/uniffi")
val debugJniLibsDir = layout.buildDirectory.dir("generated/cargo-ndk/debug/jniLibs")
val releaseJniLibsDir = layout.buildDirectory.dir("generated/cargo-ndk/release/jniLibs")

android {
    namespace = "cc.makin.urldebloater.android"
    compileSdk {
        version = release(36) {
            minorApiLevel = 1
        }
    }

    defaultConfig {
        applicationId = "cc.makin.urldebloater.android"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"

        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"

        ndk {
            abiFilters += setOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro"
            )
        }
    }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_11
        targetCompatibility = JavaVersion.VERSION_11
    }
    buildFeatures {
        compose = true
    }
    sourceSets {
        getByName("debug") {
            jniLibs.directories += debugJniLibsDir.get().asFile.absolutePath
        }
        getByName("release") {
            jniLibs.directories += releaseJniLibsDir.get().asFile.absolutePath
        }
    }
}

val hostUrldebloaterLibrary = rustTargetDir.resolve("debug/liburldebloater.so")

val buildHostUrldebloater = tasks.register<Exec>("buildHostUrldebloater") {
    group = "build"
    description = "Builds the host library used by UniFFI bindgen."
    workingDir(rustWorkspaceDir)
    inputs.files(rustInputs)
    outputs.file(hostUrldebloaterLibrary)
    commandLine("cargo", "build", "-p", "urldebloater-android", "--lib")
}

abstract class GenerateUniffiBindingsTask : Exec() {
    @get:OutputDirectory
    abstract val outputDir: DirectoryProperty
}

val generateUniffiBindings = tasks.register<GenerateUniffiBindingsTask>("generateUniffiBindings") {
    group = "build"
    description = "Generates Kotlin bindings from the host urldebloater library."
    dependsOn(buildHostUrldebloater)
    workingDir(rustWorkspaceDir)
    outputDir.set(uniffiOutputDir)
    inputs.files(rustInputs)
    inputs.file(hostUrldebloaterLibrary)
    commandLine(
        "cargo", "run", "-p", "urldebloater-android",
        "--features", "cli",
        "--bin", "uniffi-bindgen",
        "--",
        "generate",
        "--library", hostUrldebloaterLibrary.absolutePath,
        "--language", "kotlin",
        "--out-dir", outputDir.get().asFile.absolutePath,
    )
    doFirst {
        outputDir.get().asFile.deleteRecursively()
    }
}

androidComponents {
    onVariants { variant ->
        variant.sources.kotlin?.addGeneratedSourceDirectory(
            generateUniffiBindings,
            GenerateUniffiBindingsTask::outputDir,
        )
    }
}

val buildRustDebug = tasks.register<Exec>("buildRustDebug") {
    group = "build"
    description = "Builds debug native libraries for the supported Android ABIs."
    workingDir(rustWorkspaceDir)
    inputs.files(rustInputs)
    outputs.dir(debugJniLibsDir)
    commandLine(
        "cargo", "ndk",
        "--target", "arm64-v8a",
        "--target", "x86_64",
        "--platform", "24",
        "--output-dir", debugJniLibsDir.get().asFile.absolutePath,
        "build", "-p", "urldebloater-android", "--lib",
    )
}

val buildRustRelease = tasks.register<Exec>("buildRustRelease") {
    group = "build"
    description = "Builds release native libraries for the supported Android ABIs."
    workingDir(rustWorkspaceDir)
    inputs.files(rustInputs)
    outputs.dir(releaseJniLibsDir)
    commandLine(
        "cargo", "ndk",
        "--target", "arm64-v8a",
        "--target", "x86_64",
        "--platform", "24",
        "--output-dir", releaseJniLibsDir.get().asFile.absolutePath,
        "build", "-p", "urldebloater-android", "--lib", "--release",
    )
}

tasks.named("preBuild") {
    dependsOn(generateUniffiBindings)
}
tasks.configureEach {
    when (name) {
        "preDebugBuild" -> dependsOn(buildRustDebug)
        "preReleaseBuild" -> dependsOn(buildRustRelease)
    }
}

dependencies {
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.jna) {
        artifact {
            type = "aar"
        }
    }
    implementation(libs.kotlinx.coroutines.core)
    implementation(libs.rustls.platform.verifier)
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.material3.adaptive.navigation.suite)
    implementation(libs.androidx.compose.ui)
    implementation(libs.androidx.compose.ui.graphics)
    implementation(libs.androidx.compose.ui.tooling.preview)
    implementation(libs.androidx.core.ktx)
    implementation(libs.androidx.lifecycle.runtime.ktx)
    testImplementation(libs.junit)
    androidTestImplementation(platform(libs.androidx.compose.bom))
    androidTestImplementation(libs.androidx.compose.ui.test.junit4)
    androidTestImplementation(libs.androidx.espresso.core)
    androidTestImplementation(libs.androidx.junit)
    debugImplementation(libs.androidx.compose.ui.test.manifest)
    debugImplementation(libs.androidx.compose.ui.tooling)
}