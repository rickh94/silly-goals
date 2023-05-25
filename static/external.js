async function startLogin() {
  const res = await fetch("/webauthn/login");
  const loginOptions = await res.json();
  let loginInfo;
  try {
    loginInfo = await SimpleWebAuthnBrowser.startAuthentication(loginOptions.publicKey);
  } catch (error) {
    console.error(error);
    Alpine.store('notification')
  }

  const verificationResp = await fetch("/webauthn/login", {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json'
    },
    body: JSON.stringify(loginInfo)
  })

  if (verificationResp.status == 200) {
    window.location.replace("/dashboard");
  } else {
    Alpine.store('notification').show("Auth Failure","Could not authenticate, please try again", 'failure');
    window.location.replace("/login");
  }
}
