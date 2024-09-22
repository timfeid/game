export function setTheme(theme: 'light' | 'dark' | undefined) {
	if (!theme) {
		return localStorage.removeItem('theme');
	}
	localStorage.theme = theme;

	if (
		localStorage.theme === 'dark' ||
		(!('theme' in localStorage) && window.matchMedia('(prefers-color-scheme: dark)').matches)
	) {
		document.documentElement.classList.add('dark');
	} else {
		document.documentElement.classList.remove('dark');
	}
}
