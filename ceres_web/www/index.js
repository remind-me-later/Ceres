import * as wasm from "wasm-ceres";

var emulator = wasm.init_emulator();

console.log("Finished");

let canvas = document.getElementById("myCanvas"); 
let ctx = canvas.getContext("2d");

const sample_rate = 48000;

let start = undefined;

function step(timestamp) {
    if (start === undefined) {
        start = timestamp;
    }

    const elapsed = timestamp - start;
    start = timestamp;    

    for(let i = 0; i < elapsed * sample_rate / 1000; ++i)  {
        wasm.run_sample(emulator);
    }

    let fb = wasm.get_framebuffer(emulator);
    let image = new ImageData(new Uint8ClampedArray(fb), 160, 144);

    ctx.putImageData(image, 0, 0);

    window.requestAnimationFrame(step);
}

let input_file_button = document.getElementById("load_btn");
input_file_button.addEventListener('change', function() {
    var reader = new FileReader();
    reader.onload = function() {
        let arrayBuffer = this.result;
        let array = new Uint8Array(arrayBuffer);

        wasm.destroy_emulator(emulator);
        emulator = wasm.init_emulator_with_rom(array);

        console.log(array);
    }
    reader.readAsArrayBuffer(this.files[0]);
}, false);

let gb_key_map = [];

gb_key_map['a'] = 0x02;
gb_key_map['s'] = 0x08;
gb_key_map['d'] = 0x01;
gb_key_map['w'] = 0x04;

gb_key_map['m'] = 0x80;
gb_key_map['n'] = 0x40;

gb_key_map['k'] = 0x10;
gb_key_map['l'] = 0x20;

document.addEventListener("keydown", (e) => {
    let n = gb_key_map[e.key];
    if (n != undefined) {
        wasm.press_button(emulator,  n);
    }
}, false)

document.addEventListener("keyup", (e) => {
    let n = gb_key_map[e.key];
    if (n != undefined) {
        wasm.release_button(emulator,  n);
    }
}, false)


window.requestAnimationFrame(step);
