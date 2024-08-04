# zow.

> *The `zo` templating language.*

## syntax.

This code creates a `native` window using the power of the `gpu` via [`wgpu`](https://github.com/gfx-rs/wgpu) and [`egui`](https://github.com/emilk/egui) as a graphical user interface. The target attribute can be `native` or `web` depending of requirements.

```html
<script lang="zo">
  imu hi: str = "hi there!";
</script>

<window target="native|web">
  <h1>{hi}</h1>
</window>
```
