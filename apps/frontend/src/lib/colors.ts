const colors = ['#C7115A', '#9F4576', '#006853', '#6C17A6', '#255FD1', '#00778F', '#D9461A'];

export function getColor(str: string) {
	const seed = str
		.split('')
		.filter(Boolean)
		.map((c) => c.toLowerCase().charCodeAt(0) - 97)
		.reduce((a, b) => a + b, 0);

	return hexToRgb(colors[seed % colors.length] || '#C7115A');
}

function hexToRgb(hex: string) {
	const bigint = parseInt(hex.substring(1), 16);
	const r = (bigint >> 16) & 255;
	const g = (bigint >> 8) & 255;
	const b = bigint & 255;

	return { r, g, b };
}
