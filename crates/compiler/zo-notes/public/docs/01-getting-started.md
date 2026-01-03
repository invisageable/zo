Chapter 1: Getting Started

    Goal: The "Handshake."

    Content: This is the chapter we already wrote. It introduces the core philosophy ("The Player Journey"), the "Speed Demon" promise, and presents both a "Hello, World!" for the console and a "Hello, UI!" to immediately showcase zo's unique templating superpower.
    
    Goal: To make a new developer feel powerful, instantly.

    Implementation: The landing page of the documentation is a single, self-contained main.zo file that uses zo's most exciting features in a simple way. It must include a template, a simple calculation, and a #dom directive. The user should be able to copy it, run zo run, and see a result in under 30 seconds.

    Example:
   
```zo    
fun main() {
  imu name = "World";
  imu my_app ::= <div><h1>Hello, {name}!</h1></div>;
  #dom my_app;
}
```

This single example teaches variables, templating, and compilation directives in one go. It is a complete story in miniature.