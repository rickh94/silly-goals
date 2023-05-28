function prepareDeleteGoal(groupId, goalId) {
  return async () => {
    try {
      const res = await fetch(`/groups/${groupId}/goals/${goalId}`, {
        method: 'DELETE'
      });

      if (res.ok) {
        htmx.ajax('GET', `/groups/${groupId}`, "#main-content")
        Alpine.store('notification').show('Delete Succeeded', 'Deleted your goal', 'success');
      } else {
        Alpine.store('notification').show('Delete Failed', 'Could not delete your goal', 'failure');
      }
    } catch (err) {
      console.log(err);
      Alpine.store('notification').show('Delete Failed', 'Could not delete your goal', 'failure');
    }
  }

}

function confirmDeleteGoal(element) {
  const groupId = element.dataset.groupId;
  const goalId = element.dataset.goalId;
  const title = element.dataset.title;

  Alpine.store('confirm').show('Delete Goal',
    `Are you sure you want to delete ${title}?`,
    `Delete ${title}`,
    prepareDeleteGoal(groupId, goalId),
  );
}


function prepareDeleteGroup(groupId) {
  return async () => {
    try {
      const res = await fetch(`/groups/${groupId}`, {
        method: 'DELETE'
      });

      if (res.ok) {
        htmx.ajax('GET', '/dashboard', "#main-content");
        window.location.replace("/dashboard");
        document.getElementById(`group-nav-link-${groupId}`).remove()
        Alpine.store('notification').show(
          'Group Deleted',
          'Your group and all its goals have been deleted.',
          'success'
        );
      } else {
        Alpine.store('notification').show(
          'Delete Failed',
          'Your group could not be deleted. Try again later',
          'failure'
        );
      }
    } catch (err) {
      console.log(err);
      Alpine.store('notification').show(
        'Delete Failed',
        'Your group could not be deleted. Try again later',
        'failure'
      );
    }
  }
}

function confirmDeleteGroup(element) {
  const groupId = element.dataset.groupId;
  const title = element.dataset.title;

  Alpine.store('confirm').show(
    'Delete Group',
    `Are you sure you want to delete ${title} and all the associated goals?`,
    `Delete ${title}`,
    prepareDeleteGroup(groupId),
  );
}


async function deleteAccount() {
  try {
    const res = await fetch('/profile/delete', {
      method: 'POST',
    });
    if (res.ok) {
      Alpine.store('notification')
        .show(
          'Account Deleted',
          'Your account has been deleted and your data has been wiped',
        )
      window.location.replace('/');
    } else {
      Alpine.store('notification')
        .show('Delete Failed',
          'Could not delete your account, please try again',
          'failure',
          false);
    }
  } catch (err) {
    console.log(err);
    Alpine.store('notification')
      .show('Delete Failed',
        'Could not delete your account, please try again',
        'failure',
        false);
  }
}

function confirmDeleteAccount() {
  Alpine.store('confirm').show('Delete Your Account',
    'Are you sure you want to delete your account and all you goals and groups?',
    'Delete My Account',
    deleteAccount,
  );
}

function displayDate(datestring) {
  return (new Date(datestring)).toDateString();
}


async function updateGoalStage(event, droppedOn) {
  const moving = document.getElementById(event.dataTransfer.getData('text/plain'));
  moving.remove();
  droppedOn.querySelector('.goal-list').prepend(moving);
  const newStage = droppedOn.dataset.stage;
  const oldStage = moving.dataset.stage;
  const goalId = moving.dataset.goalId;
  removeDraggingPlaceholder(oldStage, moving.id);
  const groupId = moving.dataset.groupId;

  try {
    const res = await fetch(
      `/groups/${groupId}/goals/${goalId}/stage?stage=${newStage}`,
      {
        method: "PATCH",
      }
    )

    if (res.ok) {
      // There's a little conditional rendering based on what stage we're in,
      // it's simpler to have the server do it and swap it in after. Basically
      // discount htmx swap.
      moving.outerHTML = await res.text();
      Alpine.store('notification').show('Update Successful', 'Goal stage updated');
    } else {
      moving.remove();
      const putBack = document.getElementById(`stage-${oldStage}`);
      putBack.querySelector('.goal-list').prepend(moving);
      Alpine.store('notification').show('Update Failed', "Could not update goal", 'failure');
    }

  } catch (err) {
    console.log(err);
    moving.remove();
    const putBack = document.getElementById(`stage-${oldStage}`);
    putBack.querySelector('.goal-list').prepend(moving);
    Alpine.store('notification').show('Update Failed', "Could not update goal", 'failure');
  }
}

function removeDraggingPlaceholder(stage, id) {
  document.getElementById(`dragging-${stage}-${id}`).remove();
}

function createDraggingPlaceholder(stage, id) {
  let div = document.createElement('div');
  div.className = "relative border px-3 py-2 shadow-sm border-orange-500 bg-orange-300 opacity-75 h-16 rounded";
  div.id = `dragging-${stage}-${id}`;
  return div;
}

function startDragging(event, dragging) {
  event.dataTransfer.effectAllowed = 'move';
  event.dataTransfer.setData('text/plain', dragging.id);
}

function insertPlaceholder(dragging) {
  const stage = dragging.dataset.stage;
  document.getElementById(`list-stage-${stage}`).prepend(createDraggingPlaceholder(stage, dragging.id));
}


async function startRegistration() {
  let res = await fetch("/webauthn/register", {
    method: 'GET',
  });

  let creationOptions = await res.json();
  console.log(creationOptions);

  let attResp;
  try {
    attResp = await SimpleWebAuthnBrowser.startRegistration(creationOptions.publicKey);
  } catch (error) {
    Alpine.store('notification').show('Registration Failed', 'Could not register device', 'failure');
    console.debug(error);
    return;
  }


  const verificationResponse = await fetch("/webauthn/register", {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(attResp),
  });

  if (verificationResponse.ok) {
    Alpine.store('notification').show('Registration Succeeded', 'You can now log in using just this device!', 'success');
  }
}

