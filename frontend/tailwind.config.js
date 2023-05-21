module.exports = {
  content: [
    './src/**/*.rs',
    './index.html',
  ],
  theme: {
    extend: {
      colors: {
        background: "rgba(51, 41, 67, 1)",
        "light-background": "#9381b1",
        text: "#f2eff5",
      },
    },
  },
  plugins: [
    require('@tailwindcss/forms'),
    require('@tailwindcss/typography'),
  ]
}
