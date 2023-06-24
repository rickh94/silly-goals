function dummyConfirm() {
  throw new Error('should be unreachable');
}
document.addEventListener('alpine:init', () => {
  Alpine.store('location', {
    path: '',

    update() {
      setTimeout(() => {

        this.path = window.location.pathname;
      }, 50);
    }
  });
  Alpine.store('location').update();

  document.addEventListener('updateLocation', () => {
    Alpine.store('location').update();
  });

  Alpine.store('notification', {
    open: false,
    title: '',
    message: '',
    variant: 'success',
    closeTimeout: null,
    resetTimeout: null,


    show(title, message, variant = 'success', autoHide = true) {
      if (this.closeTimeout) {
        clearTimeout(this.closeTimeout);
        this.closeTimeout = null;
      }
      if (this.closeTimeout) {
        clearTimeout(this.closeTimeout);
        this.closeTimeout = null;
      }
      this.title = title;
      this.message = message;
      this.open = true;
      this.variant = variant;

      if (autoHide) {
        this.clearTimeout = setTimeout(() => {
          this.close();
        }, 4000);
      }
    },

    close() {
      this.open = false;
      this.resetTimeout = setTimeout(() => {
        this.title = '';
        this.message = '';
      }, 300);
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
      setTimeout(() => {
        this.title = '';
        this.message = '';
        this.confirmText = '';
        this.onConfirm = dummyConfirm;
      }, 300);
    },
  })

  document.addEventListener('notify', event => {
    Alpine.store('notification').show(event.detail.title, event.detail.message, event.detail.variant, event.detail.autoHide);
  })
})
