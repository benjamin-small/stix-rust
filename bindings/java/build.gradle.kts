plugins {
    java
}

repositories { mavenCentral() }

dependencies {
    implementation("com.fasterxml.jackson.core:jackson-databind:2.17.1")
    testImplementation(platform("org.junit:junit-bom:5.10.2"))
    testImplementation("org.junit.jupiter:junit-jupiter")
    testRuntimeOnly("org.junit.platform:junit-platform-launcher")
}

val cargoBuild by tasks.registering(Exec::class) {
    workingDir = file("rust")
    commandLine("cargo", "build", "--release")
}

tasks.test {
    dependsOn(cargoBuild)
    useJUnitPlatform()
    systemProperty("java.library.path", file("rust/target/release").absolutePath)
}
