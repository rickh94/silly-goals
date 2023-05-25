function dummyConfirm() {
  alert('Should be unreachable');
}

document.addEventListener('alpine:init', () => {
  Alpine.store('notification', {
    open: false,
    title: '',
    message: '',
    variant: 'success',

    show(title, message, variant = 'success', autoHide = true) {
      this.title = title;
      this.message = message;
      this.open = true;
      this.variant = variant;

      if (autoHide) {
        setTimeout(() => {
          this.close();
        }, 4000);
      }
    },

    close() {
      this.open = false;
      this.title = '';
      this.message = '';
    }
  });

  Alpine.store('confirm', {
    open: false,
    title: '',
    message: '',
    confirmText: '',
    onConfirm: dummyConfirm,

    show(title, message, confirmText, onConfirm) {
      this.title = title;
      this.message = message;
      this.confirmText = confirmText;
      this.onConfirm = onConfirm;
      this.open = true;
    },

    close() {
      this.open = false;
      this.title = '';
      this.message = '';
      this.confirmText = '';
      this.onConfirm = dummyConfirm;
    },
  })
})
