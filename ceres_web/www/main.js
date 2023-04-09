import { instantiate } from "./rs_lib.generated.js";

const gb = await instantiate();

var emulator = gb.init_emulator();
var canvas = document.getElementById("myCanvas");
var ctx = canvas.getContext("2d");

var start = undefined;

const sample_rate = 48000;

let gb_key_map = [];
gb_key_map["a"] = 0x02;
gb_key_map["s"] = 0x08;
gb_key_map["d"] = 0x01;
gb_key_map["w"] = 0x04;

gb_key_map["m"] = 0x80;
gb_key_map["n"] = 0x40;

gb_key_map["k"] = 0x10;
gb_key_map["l"] = 0x20;

function step(timestamp) {
  if (start === undefined) {
    start = timestamp;
  }

  const elapsed = timestamp - start;
  start = timestamp;

  let num_samples_to_run = Math.round((elapsed * sample_rate) / 1000);
  gb.run_n_samples(emulator, num_samples_to_run);

  let fb = gb.get_framebuffer(emulator);
  let image_data = new ImageData(new Uint8ClampedArray(fb), 160, 144);

  ctx.putImageData(image_data, 0, 0);

  window.requestAnimationFrame(step);
}

function main() {
  // Input file
  let input_file_button = document.getElementById("load_btn");
  input_file_button.addEventListener(
    "change",
    function () {
      var reader = new FileReader();
      reader.onload = function () {
        let arrayBuffer = this.result;
        let array = new Uint8Array(arrayBuffer);

        gb.free_gb(emulator);
        emulator = gb.init_emulator_with_rom(array);

        console.log(array);
      };
      reader.readAsArrayBuffer(this.files[0]);
    },
    false
  );

  // Button presses
  document.addEventListener(
    "keydown",
    (e) => {
      let n = gb_key_map[e.key];
      if (n != undefined) {
        gb.press_button(emulator, n);
      }
    },
    false
  );

  document.addEventListener(
    "keyup",
    (e) => {
      let n = gb_key_map[e.key];
      if (n != undefined) {
        gb.release_button(emulator, n);
      }
    },
    false
  );

  // Animation display
  window.requestAnimationFrame(step);
}

main();
