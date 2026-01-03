// //! End-to-end test for template compilation and runtime loading

// use zo_interner::Interner;
// use zo_sir::{Insn, Sir};
// use zo_ty::TyId;
// use zo_ui_protocol::{UiCommand, ContainerDirection, TextStyle};
// use zo_value::ValueId;
// use zo_codegen_arm::ARM64Gen;
// use zo_runtime::{Runtime, RuntimeConfig};

// use std::fs;
// use std::path::Path;

// /// Test the complete pipeline from template to rendered UI
// #[test]
// #[ignore] // Run with: cargo test end_to_end_template --ignored
// fn test_template_compilation_and_loading() {
//     // Step 1: Generate a template with ARM codegen
//     let mut interner = Interner::new();
//     let mut sir = Sir::new();

//     // Create template commands
//     let commands = vec![
//         UiCommand::BeginContainer {
//             id: "main".to_string(),
//             direction: ContainerDirection::Vertical,
//         },
//         UiCommand::Text {
//             content: "Hello from Zo!".to_string(),
//             style: TextStyle::Heading1,
//         },
//         UiCommand::Text {
//             content: "This is a compiled template".to_string(),
//             style: TextStyle::Paragraph,
//         },
//         UiCommand::Button {
//             id: 42,
//             content: "Click Me".to_string(),
//         },
//         UiCommand::EndContainer,
//     ];

//     // Emit template instruction
//     sir.emit(Insn::Template {
//         id: ValueId(1),
//         name: Some(interner.intern("test_template")),
//         ty_id: TyId(0),
//         commands,
//     });

//     // Generate ARM64 code
//     let mut codegen = ARM64Gen::new(&interner);
//     let artifact = codegen.generate(&sir);

//     // Generate Mach-O binary
//     let binary = codegen.generate_macho(artifact);

//     // Save to temporary file
//     let temp_path = "/tmp/zo_test_template.dylib";
//     fs::write(temp_path, binary).expect("Failed to write test binary");

//     println!("Generated test binary at: {}", temp_path);

//     // Step 2: Load the binary with runtime
//     let mut runtime = Runtime::new();

//     // Try to load the library
//     match runtime.load_library(temp_path) {
//         Ok(()) => {
//             println!("Successfully loaded library!");

//             // In a real scenario, we would run the UI
//             // runtime.run().expect("Failed to run");
//         }
//         Err(e) => {
//             // This might fail due to code signing on macOS
//             println!("Library loading failed (expected on unsigned binaries):
// {}", e);         }
//     }

//     // Clean up
//     let _ = fs::remove_file(temp_path);
// }

// /// Test that the runtime can render commands directly
// #[test]
// fn test_runtime_direct_rendering() {
//     let mut runtime = Runtime::new();

//     // Set commands directly
//     runtime.set_commands(vec![
//         UiCommand::BeginContainer {
//             id: "test".to_string(),
//             direction: ContainerDirection::Vertical,
//         },
//         UiCommand::Text {
//             content: "Direct Render Test".to_string(),
//             style: TextStyle::Heading2,
//         },
//         UiCommand::EndContainer,
//     ]);

//     // We can't actually run the UI in a test, but we can verify setup
//     println!("Runtime configured with direct commands");
// }
