async function startLogin() {
  const res = await fetch("/webauthn/login");
  const loginOptions = await res.json();
  let loginInfo;
  try {
    loginInfo = await SimpleWebAuthnBrowser.startAuthentication(loginOptions.publicKey);
  } catch (error) {
    console.error(error);
    alert("Something went wrong");
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
    alert("Could not authenticate, please try again");
    window.location.replace("/login");
  }
}
