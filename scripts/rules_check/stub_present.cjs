// Stand-in for the 'present' npm package (a millisecond timer) used by 5d-chess-js mate.js.
module.exports = () => performance.now()
