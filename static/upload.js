const UPDATE_INTERVAL_MS = 100;
const SMOOTHING = 0.3;

const form = document.getElementById('form');
const input = document.getElementById('data');
const currentElem = document.getElementById('current');
const rateElem = document.getElementById('rate');
const etaElem = document.getElementById('eta');
const maxElem = document.getElementById('max');
const progressBarElem = document.getElementById('progress');
const errorElem = document.getElementById('error');
const progressContainer = document.getElementById('progress-container');

function formatBytes(bytes) {
	if (bytes < 2048) {
		return bytes.toString() + ' B';
	} else if (bytes < 2048 * 1024) {
		return (bytes / 1024).toPrecision(4) + ' KiB';
	} else if (bytes < 2048 * 1024 ** 2) {
		return (bytes / 1024 ** 2).toPrecision(4) + ' MiB';
	} else {
		return (bytes / 1024 ** 3).toPrecision(4) + ' GiB';
	}
}

function formatDuration(seconds) {
	let [days, hrs, mins, secs] = [
		Math.floor(seconds / 86400),
		Math.floor(seconds / 3600) % 24,
		Math.floor(seconds / 60) % 60,
		seconds % 60,
	];
	if (days == 0 && hrs == 0) {
		// mm:ss
		return `${mins}:${secs.toString().padStart(2, '0')}`;
	} else if (days == 0) {
		// hh:mm:ss
		return `${hrs}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
	} else {
		// d:hh:mm:ss
		return `${days}:${hrs.toString().padStart(2, '0')}:${mins.toString().padStart(2, '0')}:${secs.toString().padStart(2, '0')}`;
	}
}

// based on XHR solution from https://stackoverflow.com/a/69400632
async function upload() {
	const formData = new FormData();
	let sum = 0;
	for (const f of input.files) {
		formData.append('data', f);
		sum += f.size;
	}
	if (sum > uploadLimit) {
		throw new Error(`Exceeded upload limit: ${formatBytes(sum)} > ${formatBytes(uploadLimit)}`);
	}

	const {instance: {exports}} = await WebAssembly.instantiateStreaming(await fetch('/xxhash.wasm'));
	exports.init(BigInt(69420));
	console.log(exports.buffer);
	const memAsBytes = new Uint8Array(exports.memory.buffer);
	const reader = input.files[0].stream().getReader();
	const start = Date.now();
	while (true) {
		const {value: chunk, done} = await reader.read();
		if (done) break;

		let offset = 0;
		while (offset < chunk.length) {
			const toCopy = Math.min(65536, chunk.length - offset);
			memAsBytes.set(new Uint8Array(chunk.buffer, offset, toCopy), exports.buffer.value);
			exports.update(exports.buffer.value, toCopy);
			offset += toCopy;
		}
	}
	console.log(BigInt.asUintN(64, exports.digest()).toString(16).padStart(16, '0'));
	const speed = 1000 * input.files[0].size / (Date.now() - start) / 1e6;
	console.log(speed);

	maxElem.textContent = formatBytes(sum);
	const xhr = new XMLHttpRequest();
	await new Promise((resolve, reject) => {
		let lastUpdate = Date.now();
		let lastUpdateBytes = 0;
		let bytesPerS = -1;
		xhr.upload.addEventListener('progress', (e) => {
			if (e.lengthComputable) {
				const now = Date.now();
				const elapsed = now - lastUpdate;
				if (elapsed > UPDATE_INTERVAL_MS) {
					lastUpdate = now;
					const bytesSinceUpdate = e.loaded - lastUpdateBytes;
					lastUpdateBytes = e.loaded;
					const newRateEstimate = bytesSinceUpdate / (elapsed / 1000);
					if (bytesPerS < 0) {
						bytesPerS = newRateEstimate;
					} else {
						bytesPerS = SMOOTHING * newRateEstimate + (1 - SMOOTHING) * bytesPerS;
					}

					currentElem.textContent = formatBytes(e.loaded);
					rateElem.textContent = formatBytes(Math.round(bytesPerS));
					const timeRemaining = Math.round((e.loaded >= e.total || bytesPerS == 0) ? 0 : ((e.total - e.loaded) / bytesPerS));
					etaElem.textContent = formatDuration(timeRemaining);
				}
				progressBarElem.value = e.loaded / e.total;
				progressBarElem.textContent = `${(100 * e.loaded / e.total).toPrecision(3)}%`;
			}
		});
		xhr.addEventListener('loadend', () => {
			if (xhr.readyState == XMLHttpRequest.DONE) {
				if (xhr.status == 200) {
					resolve();
				} else {
					reject(new Error(`HTTP error: ${xhr.status} ${xhr.statusText}`));
				}
			} else {
				reject(new Error('Request did not complete'));
			}
		});
		xhr.open('POST', '/upload', true);
		xhr.send(formData);
	});
}

form.onsubmit = (event) => {
	event.preventDefault();
	errorElem.textContent = '';
	progressContainer.style.display = 'block';
	upload()
		.then(() => {
			errorElem.textContent = 'Your files have been uploaded.';
		})
		.catch((err) => {
			errorElem.textContent = err.toString();
		}).finally(() => {
			progressContainer.style.display = 'none';
		});
	return false;
};
