const path = require('path');
const webpack = require('webpack');

module.exports = {
    mode: 'production',
    entry: './src/index.js',
    output: {
        filename: 'jsdeps.js',
        path: path.resolve(__dirname, 'dist'),
        library: 'JsDeps',
        libraryTarget: 'window',
    },
    resolve: {
        extensions: ['.js']
    },
    module: {
        rules: [
            {
                test: /\.css$/i,
                sideEffects: true,
                use: ['style-loader', 'css-loader'],
            },
        ],
    },
    plugins: [
        new webpack.optimize.LimitChunkCountPlugin({ maxChunks: 1 }),
    ],
};
