/** @type {import('tailwindcss').Config} */
module.exports = {
  content: ['*.html', '**/*.html', '**/*.rs'],
  theme: {
    extend: {},
  },
  plugins: [
    require(
      '@tailwindcss/forms'
    )
  ],
}

