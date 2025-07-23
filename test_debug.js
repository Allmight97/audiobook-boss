// Debug file path access in browser
const input = document.createElement('input');
input.type = 'file';
input.addEventListener('change', (e) => {
    const file = e.target.files[0];
    console.log('file.path:', file.path);
    console.log('file.name:', file.name);
    console.log('file.webkitRelativePath:', file.webkitRelativePath);
});
